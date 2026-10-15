//! 应用持有的导入副本；外部资源引用从不包含原路径。
use crate::{Guidance, GuidanceKind, GuidanceSource};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tauri::http::{Request, Response, StatusCode};

// 单文件页面主要承载轻量指引，也为内嵌图片保留足够空间。
const MAX_HTML_BYTES: u64 = 8 * 1024 * 1024;

pub struct GuidanceResources {
    directory: PathBuf,
}

pub struct ImportedContent {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

pub(crate) fn valid_id(id: &str) -> bool {
    id.strip_prefix("local-").is_some_and(|value| {
        value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

pub(crate) fn settings_guidance(settings: &crate::Settings) -> impl Iterator<Item = &Guidance> {
    settings
        .tips
        .iter()
        .filter_map(|entry| entry.guidance.as_ref())
        .chain(
            settings
                .schedules
                .iter()
                .flat_map(|schedule| &schedule.periods)
                .flat_map(|period| &period.overrides)
                .filter_map(|adjustment| adjustment.guidance.as_ref()),
        )
}

impl GuidanceResources {
    pub fn open(directory: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&directory).map_err(|_| "resource-unavailable")?;
        Ok(Self { directory })
    }

    pub fn import_video(&self, source: &Path) -> Result<Guidance, String> {
        let mut input = File::open(source).map_err(|_| "import-failed")?;
        if !input.metadata().is_ok_and(|metadata| metadata.is_file()) {
            return Err("import-failed".into());
        }
        let mut header = [0; 12];
        input
            .read_exact(&mut header)
            .map_err(|_| "unsupported-video")?;
        if &header[4..8] != b"ftyp" {
            return Err("unsupported-video".into());
        }
        let id = format!("local-{:032x}", fastrand::u128(..));
        let temporary = self.directory.join(format!("{id}.tmp"));
        let destination = self.directory.join(format!("{id}.mp4"));
        let result = (|| -> std::io::Result<()> {
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            output.write_all(&header)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
            fs::rename(&temporary, &destination)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
            return Err("import-failed".into());
        }
        Ok(Guidance {
            kind: GuidanceKind::Video,
            source: GuidanceSource::Local { id },
        })
    }

    pub fn import_html(&self, source: &Path) -> Result<Guidance, String> {
        if !source
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                ["html", "htm"].contains(&extension.to_ascii_lowercase().as_str())
            })
        {
            return Err("unsupported-html".into());
        }
        let input = File::open(source).map_err(|_| "import-failed")?;
        if !input.metadata().is_ok_and(|metadata| metadata.is_file()) {
            return Err("import-failed".into());
        }
        let mut bytes = Vec::new();
        input
            .take(MAX_HTML_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "import-failed")?;
        if !valid_html(&bytes) {
            return Err("unsupported-html".into());
        }
        let id = format!("local-{:032x}", fastrand::u128(..));
        let temporary = self.directory.join(format!("{id}.tmp"));
        let destination = self.directory.join(format!("{id}.html"));
        let result = (|| -> std::io::Result<()> {
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            output.write_all(&bytes)?;
            output.sync_all()?;
            fs::rename(&temporary, &destination)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
            return Err("import-failed".into());
        }
        Ok(Guidance {
            kind: GuidanceKind::Html,
            source: GuidanceSource::Local { id },
        })
    }

    fn file(&self, guidance: &Guidance) -> Result<File, String> {
        let GuidanceSource::Local { id } = &guidance.source else {
            return Err("resource-unavailable".into());
        };
        if !valid_id(id) {
            return Err("resource-unavailable".into());
        }
        let extension = match guidance.kind {
            GuidanceKind::Video => "mp4",
            GuidanceKind::Html => "html",
        };
        let path = self.directory.join(format!("{id}.{extension}"));
        if !fs::symlink_metadata(&path)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        {
            return Err("resource-unavailable".into());
        }
        let mut file = File::open(&path).map_err(|_| "resource-unavailable")?;
        match guidance.kind {
            GuidanceKind::Video => {
                let mut header = [0; 12];
                file.read_exact(&mut header)
                    .map_err(|_| "resource-unavailable")?;
                if &header[4..8] != b"ftyp" {
                    return Err("resource-unavailable".into());
                }
            }
            GuidanceKind::Html => {
                let mut bytes = Vec::new();
                (&mut file)
                    .take(MAX_HTML_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "resource-unavailable")?;
                if !valid_html(&bytes) {
                    return Err("resource-unavailable".into());
                }
            }
        }
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(0))
            .map_err(|_| "resource-unavailable")?;
        Ok(file)
    }

    /// 按已登记来源读取副本，后续 HTML 沿用此入口，不赋予用户页面原生协议权限。
    pub fn read(&self, guidance: &Guidance) -> Result<ImportedContent, String> {
        let mut bytes = Vec::new();
        self.file(guidance)?
            .read_to_end(&mut bytes)
            .map_err(|_| "resource-unavailable")?;
        Ok(ImportedContent {
            mime: match guidance.kind {
                GuidanceKind::Video => "video/mp4",
                GuidanceKind::Html => "text/html; charset=utf-8",
            },
            bytes,
        })
    }

    /// 保存设置前确认所选本地副本可读取；失败时调用者保留整份旧配置。
    pub fn validate_settings(&self, settings: &crate::Settings) -> Result<(), String> {
        let guidance = settings_guidance(settings);
        for guidance in guidance {
            if matches!(guidance.source, GuidanceSource::Local { .. }) {
                self.file(guidance)?;
            }
        }
        Ok(())
    }

    /// 启动时先由 TimerApp 校验恢复进度，再联合全部设置和有效当前轮回收。
    /// 编辑中的导入在本次运行内不回收，避免尚未保存的副本提前失效。
    pub fn retain_for(
        &self,
        settings: &crate::Settings,
        current: &crate::timer::TimerInfo,
    ) -> Result<(), String> {
        let mut keep = std::collections::HashSet::new();
        let mut remember = |guidance: &Guidance| {
            if let GuidanceSource::Local { id } = &guidance.source {
                keep.insert(id.clone());
            }
        };
        for guidance in settings_guidance(settings) {
            remember(guidance);
        }
        if let Some(guidance) = &current.guidance {
            remember(guidance);
        }
        for entry in fs::read_dir(&self.directory).map_err(|_| "resource-unavailable")? {
            let entry = entry.map_err(|_| "resource-unavailable")?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some((id, extension)) = name.rsplit_once('.') else {
                continue;
            };
            if valid_id(id) && ["mp4", "html", "tmp"].contains(&extension) && !keep.contains(id) {
                fs::remove_file(entry.path()).map_err(|_| "resource-unavailable")?;
            }
        }
        Ok(())
    }

    pub fn video_response(&self, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
        use std::io::{Seek, SeekFrom};
        use tauri::http::Method;
        let response = || {
            Response::builder()
                .header("x-content-type-options", "nosniff")
                .header("access-control-allow-origin", "*")
                .header(
                    "access-control-expose-headers",
                    "content-range, accept-ranges, content-length",
                )
        };
        if request.method() != Method::GET && request.method() != Method::HEAD {
            return response()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .header("allow", "GET, HEAD")
                .body(Vec::new())
                .unwrap();
        }
        let id = request.uri().path().strip_prefix('/').unwrap_or("");
        let guidance = Guidance {
            kind: GuidanceKind::Video,
            source: GuidanceSource::Local { id: id.into() },
        };
        let Ok(mut file) = self.file(&guidance) else {
            return response()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .unwrap();
        };
        let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
            return response()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .unwrap();
        };
        let range = request.headers().get("range").map(|value| {
            let value = value.to_str().ok()?.strip_prefix("bytes=")?;
            let (first, last) = value.split_once('-')?;
            if !first
                .bytes()
                .chain(last.bytes())
                .all(|byte| byte.is_ascii_digit())
            {
                return None;
            }
            let (start, end) = if first.is_empty() {
                let suffix = last.parse::<u64>().ok()?;
                if suffix == 0 {
                    return None;
                }
                (length.saturating_sub(suffix), length.checked_sub(1)?)
            } else {
                let start = first.parse::<u64>().ok()?;
                let end = if last.is_empty() {
                    length.checked_sub(1)?
                } else {
                    last.parse::<u64>().ok()?.min(length.checked_sub(1)?)
                };
                (start, end)
            };
            if start > end || start >= length {
                return None;
            }
            // 原生播放器按区间持续读取，不为单次 Range 分配整个大文件。
            Some((start, end.min(start.saturating_add(1024 * 1024 - 1))))
        });
        if range == Some(None) {
            return response()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header("content-range", format!("bytes */{length}"))
                .body(Vec::new())
                .unwrap();
        }
        let (start, end) = range.flatten().unwrap_or((0, length.saturating_sub(1)));
        let count = end - start + 1;
        let mut builder = response()
            .header("content-type", "video/mp4")
            .header("accept-ranges", "bytes")
            .header("content-length", count);
        if range.is_some() {
            builder = builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header("content-range", format!("bytes {start}-{end}/{length}"));
        }
        let mut bytes = Vec::new();
        if request.method() != Method::HEAD
            && (file.seek(SeekFrom::Start(start)).is_err()
                || file.take(count).read_to_end(&mut bytes).is_err()
                || bytes.len() as u64 != count)
        {
            return response()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .unwrap();
        }
        builder.body(bytes).unwrap()
    }
}

fn valid_html(bytes: &[u8]) -> bool {
    bytes.len() <= MAX_HTML_BYTES as usize
        && !bytes.contains(&0)
        && std::str::from_utf8(bytes).is_ok_and(|text| !text.trim().is_empty())
}
