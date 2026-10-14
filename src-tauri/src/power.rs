//! 原生休眠输入。只报告实际电脑睡眠，不把屏幕关闭当成电脑休眠。
use std::sync::{mpsc::Sender, Arc, Mutex};
use std::time::Duration;

#[derive(Debug)]
pub enum PowerEvent {
    Suspend { idle_secs: u64 },
    Resume { elapsed_secs: u64, idle_secs: u64 },
}

struct PowerPort {
    sender: Sender<PowerEvent>,
    clock: native::ContinuousClock,
    sleeping_since: Mutex<Option<Duration>>,
}

impl PowerPort {
    fn new(sender: Sender<PowerEvent>) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            sender,
            clock: native::ContinuousClock::new()?,
            sleeping_since: Mutex::new(None),
        }))
    }

    fn suspend(&self) {
        let Ok(mut sleeping) = self.sleeping_since.lock() else {
            return;
        };
        if sleeping.is_none() {
            *sleeping = Some(self.clock.now());
            let _ = self.sender.send(PowerEvent::Suspend {
                idle_secs: crate::platform::idle_secs(),
            });
        }
    }

    fn resume(&self) {
        let Ok(mut sleeping) = self.sleeping_since.lock() else {
            return;
        };
        if let Some(started) = sleeping.take() {
            let elapsed_secs = self.clock.now().saturating_sub(started).as_secs();
            let _ = self.sender.send(PowerEvent::Resume {
                elapsed_secs,
                idle_secs: crate::platform::idle_secs(),
            });
        }
    }
}

pub use native::PowerMonitor;

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, ProtocolObject};
    use objc2_app_kit::{
        NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillSleepNotification,
    };
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol};
    use std::ptr::NonNull;

    #[repr(C)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }

    extern "C" {
        fn mach_timebase_info(info: *mut Timebase) -> i32;
        fn mach_continuous_time() -> u64;
    }

    pub(super) struct ContinuousClock(Timebase);

    impl ContinuousClock {
        pub(super) fn new() -> Result<Self, String> {
            let mut info = Timebase { numer: 0, denom: 0 };
            // SAFETY: libSystem 只向有效的输出指针写入两个 u32 字段。
            let result = unsafe { mach_timebase_info(&mut info) };
            if result != 0 || info.denom == 0 {
                return Err(format!("无法读取系统连续时钟：{result}"));
            }
            Ok(Self(info))
        }

        pub(super) fn now(&self) -> Duration {
            // SAFETY: 系统时钟查询无参数，也不保留任何指针。
            let ticks = unsafe { mach_continuous_time() };
            let ns = u128::from(ticks) * u128::from(self.0.numer) / u128::from(self.0.denom);
            Duration::new((ns / 1_000_000_000) as u64, (ns % 1_000_000_000) as u32)
        }
    }

    /// 由 Tauri 主线程持有，退出时在同一线程移除观察者。
    pub struct PowerMonitor {
        center: Retained<NSNotificationCenter>,
        tokens: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    }

    impl PowerMonitor {
        pub fn new(sender: Sender<PowerEvent>) -> Result<Self, String> {
            if objc2::MainThreadMarker::new().is_none() {
                return Err("系统休眠观察者必须在主线程注册".into());
            }
            let port = PowerPort::new(sender)?;
            let center = NSWorkspace::sharedWorkspace().notificationCenter();
            let sleep_port = port.clone();
            let sleep = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                sleep_port.suspend();
            });
            let wake = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                port.resume();
            });
            // SAFETY: NSWorkspace 定义这些通知名；block 只捕获线程安全的
            // Rust 数据，通知中心复制并持有 block。
            let tokens = unsafe {
                vec![
                    center.addObserverForName_object_queue_usingBlock(
                        Some(NSWorkspaceWillSleepNotification),
                        None,
                        None,
                        &sleep,
                    ),
                    center.addObserverForName_object_queue_usingBlock(
                        Some(NSWorkspaceDidWakeNotification),
                        None,
                        None,
                        &wake,
                    ),
                ]
            };
            Ok(Self { center, tokens })
        }
    }

    impl Drop for PowerMonitor {
        fn drop(&mut self) {
            for token in &self.tokens {
                let observer: &AnyObject = (**token).as_ref();
                // SAFETY: token 来自同一通知中心的 block 注册，在移除时仍存活。
                unsafe { self.center.removeObserver(observer) };
            }
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Power::{
        PowerRegisterSuspendResumeNotification, PowerUnregisterSuspendResumeNotification,
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY,
    };
    use windows::Win32::System::SystemInformation::GetTickCount64;
    use windows::Win32::UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    };

    static PORT: OnceLock<Mutex<Option<Arc<PowerPort>>>> = OnceLock::new();

    pub(super) struct ContinuousClock;

    impl ContinuousClock {
        pub(super) fn new() -> Result<Self, String> {
            Ok(Self)
        }

        pub(super) fn now(&self) -> Duration {
            // SAFETY: GetTickCount64 无参数，返回值包含系统睡眠经过时间。
            Duration::from_millis(unsafe { GetTickCount64() })
        }
    }

    unsafe extern "system" fn callback(
        _context: *const c_void,
        event_type: u32,
        _setting: *const c_void,
    ) -> u32 {
        // 进程生命周期的注册表避免注销期间在途回调解引用已释放的应用指针。
        let port = PORT
            .get()
            .and_then(|registry| registry.lock().ok()?.clone());
        if let Some(port) = port {
            match event_type {
                PBT_APMSUSPEND => port.suspend(),
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => port.resume(),
                _ => {}
            }
        }
        0
    }

    pub struct PowerMonitor {
        registration: HPOWERNOTIFY,
        _parameters: Box<DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS>,
    }

    impl PowerMonitor {
        pub fn new(sender: Sender<PowerEvent>) -> Result<Self, String> {
            let port = PowerPort::new(sender)?;
            let registry = PORT.get_or_init(|| Mutex::new(None));
            {
                let mut registered = registry.lock().map_err(|error| error.to_string())?;
                if registered.is_some() {
                    return Err("系统休眠观察者已注册".into());
                }
                *registered = Some(port);
            }
            let mut parameters = Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
                Callback: Some(callback),
                Context: std::ptr::null_mut(),
            });
            let mut handle = std::ptr::null_mut();
            // SAFETY: Box 中的参数在整个注册期间存活，回调不使用 raw context。
            let result = unsafe {
                PowerRegisterSuspendResumeNotification(
                    DEVICE_NOTIFY_CALLBACK,
                    HANDLE((&mut *parameters as *mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS).cast()),
                    &mut handle,
                )
            };
            if result.0 != 0 {
                if let Ok(mut registered) = registry.lock() {
                    *registered = None;
                }
                return Err(format!("无法注册系统休眠观察者：{}", result.0));
            }
            Ok(Self {
                registration: HPOWERNOTIFY(handle as isize),
                _parameters: parameters,
            })
        }
    }

    impl Drop for PowerMonitor {
        fn drop(&mut self) {
            if let Some(registry) = PORT.get() {
                if let Ok(mut registered) = registry.lock() {
                    *registered = None;
                }
            }
            // SAFETY: 句柄来自配对的注册函数；参数在 Drop 返回前仍存活。
            let result = unsafe { PowerUnregisterSuspendResumeNotification(self.registration) };
            if result.0 != 0 {
                eprintln!("无法注销系统休眠观察者：{}", result.0);
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod native {
    use super::*;

    pub(super) struct ContinuousClock(std::time::Instant);

    impl ContinuousClock {
        pub(super) fn new() -> Result<Self, String> {
            Ok(Self(std::time::Instant::now()))
        }
        pub(super) fn now(&self) -> Duration {
            self.0.elapsed()
        }
    }

    pub struct PowerMonitor {
        _port: Arc<PowerPort>,
    }

    impl PowerMonitor {
        pub fn new(sender: Sender<PowerEvent>) -> Result<Self, String> {
            Ok(Self {
                _port: PowerPort::new(sender)?,
            })
        }
    }
}
