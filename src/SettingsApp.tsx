import { useEffect, useRef, useState, type FormEvent } from "react";
import {
  ActionIcon,
  Alert,
  Anchor,
  Button,
  Group,
  NavLink,
  NumberInput,
  ScrollArea,
  SegmentedControl,
  Select,
  Stack,
  Text,
  Textarea,
  Title,
  Tooltip,
} from "@mantine/core";
import { DatesProvider } from "@mantine/dates";
import {
  ArrowCounterClockwiseIcon,
  BellIcon,
  CalendarBlankIcon,
  CheckIcon,
  ClockIcon,
  FileVideoIcon,
  GearSixIcon,
  GlobeIcon,
  MonitorIcon,
  PlusIcon,
  PowerIcon,
  TrashIcon,
  XIcon,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "dayjs/locale/zh-cn";
import {
  AppearancePicker,
  DurationControl,
  PreferenceRow,
  dragWindow,
  plainLabel,
  type ClockRegistry,
} from "./components";
import { STR, fmt } from "./i18n";
import { GuidancePicker } from "./Guidance";
import { ScheduleEditor } from "./ScheduleEditor";
import { SchedulePreview } from "./SchedulePreview";
import {
  errorKey,
  prepareSettings,
  type Settings,
  type SettingsDraft,
  type ValidationError,
} from "./model";
import {
  refreshSettings,
  resolveLang,
  usePlatform,
  useSettings,
} from "./runtime";
import { usePreviewTheme } from "./theme";

type Section = "timer" | "schedules" | "reminders" | "general";
export default function SettingsApp() {
  const saved = useSettings(),
    platform = usePlatform();
  const [draft, setDraft] = useState<SettingsDraft>(() =>
    structuredClone(saved),
  );
  const [section, setSection] = useState<Section>("timer");
  const [dirty, setDirty] = useState(false),
    [saving, setSaving] = useState(false),
    [success, setSuccess] = useState(false);
  const [errors, setErrors] = useState<ValidationError[]>([]),
    [failure, setFailure] = useState("");
  const [imports, setImports] = useState(0),
    [preview, setPreview] = useState(false);
  const touched = useRef(new Set<keyof SettingsDraft>()),
    revision = useRef(0);
  const clocks = useRef<ClockRegistry>(new Map());
  const draftRef = useRef(draft);
  draftRef.current = draft;
  const form = useRef<HTMLFormElement>(null),
    viewport = useRef<HTMLDivElement>(null);
  const scrolls = useRef<Record<Section, number>>({
    timer: 0,
    schedules: 0,
    reminders: 0,
    general: 0,
  });
  const previewTheme = usePreviewTheme();
  const lang = resolveLang(draft.language),
    S = STR[lang];
  const busy = imports > 0;
  function mark(key: keyof SettingsDraft) {
    touched.current.add(key);
    revision.current++;
    setDirty(true);
    setSuccess(false);
    setFailure("");
    setErrors([]);
  }
  function change<K extends keyof SettingsDraft>(
    key: K,
    value: SettingsDraft[K],
  ) {
    mark(key);
    setDraft((current) => ({ ...current, [key]: value }));
  }
  function onBusy(delta: number) {
    setImports((current) => Math.max(0, current + delta));
  }
  function navigate(next: Section) {
    if (viewport.current) scrolls.current[section] = viewport.current.scrollTop;
    setSection(next);
    requestAnimationFrame(() => {
      if (viewport.current) viewport.current.scrollTop = scrolls.current[next];
      document
        .getElementById(`heading-${next}`)
        ?.focus({ preventScroll: true });
    });
  }
  useEffect(() => {
    previewTheme(draft.appearance);
  }, [draft.appearance, previewTheme]);
  useEffect(() => {
    document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
  }, [lang]);
  useEffect(() => {
    // 其他窗口修改快捷时长／大小时，同步未编辑的字段，保留当前草稿。
    setDraft((current) => {
      const next = { ...current };
      for (const key of Object.keys(saved) as (keyof Settings)[])
        if (!touched.current.has(key))
          Object.assign(next, { [key]: structuredClone(saved[key]) });
      return next;
    });
  }, [saved]);
  useEffect(() => {
    const off = listen("settings_open", () => {
      if (!touched.current.size)
        void refreshSettings().catch(() => setFailure(S.loadFailed));
    });
    return () => {
      void off.then((fn) => fn());
    };
  }, [S]);
  useEffect(() => {
    function keyboard(event: KeyboardEvent) {
      if (event.isComposing || event.defaultPrevented) return;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        if (!busy && !saving) form.current?.requestSubmit();
        return;
      }
      if (event.key !== "Escape" || preview) return;
      if ((event.target as HTMLElement).closest('[aria-expanded="true"]'))
        return;
      const popup = Array.from(
        document.querySelectorAll<HTMLElement>(
          '[role="listbox"],.mantine-Popover-dropdown',
        ),
      ).some((el) => el.getBoundingClientRect().height > 0);
      if (!popup)
        void invoke("hide_settings").catch(() => setFailure(S.operationFailed));
    }
    // 在组件关闭下拉框之前读取展开状态，第一次 Escape 只交给当前弹层。
    window.addEventListener("keydown", keyboard, true);
    return () => window.removeEventListener("keydown", keyboard, true);
  }, [busy, saving, preview, S]);
  function snapshotDraft() {
    const next = structuredClone(draftRef.current);
    for (const schedule of next.schedules)
      for (const period of schedule.periods)
        for (const field of ["start", "end"] as const) {
          const read = clocks.current.get(`${period.id}/${field}`);
          if (read) period[field] = read();
        }
    return next;
  }
  function displayErrors(issues: ValidationError[]) {
    setErrors(issues);
    const first = issues[0];
    if (first) {
      const target: Section = first.schedule_id
        ? "schedules"
        : first.field.startsWith("tips.")
          ? "reminders"
          : first.field === "widget_size"
            ? "general"
            : "timer";
      navigate(target);
      // Accordion 先展开有错轮次，再把焦点放到实际输入上。
      setTimeout(() => {
        const host = document.querySelector<HTMLElement>(
          `[data-error-key="${CSS.escape(errorKey(first))}"]`,
        );
        const input = host?.matches("input,textarea,button")
          ? host
          : host?.querySelector<HTMLElement>(
              "input:not([type=hidden]),textarea,button",
            );
        input?.focus();
        input?.scrollIntoView({ block: "nearest" });
      }, 240);
    }
    setFailure(
      issues
        .map((issue) => {
          const schedule = draftRef.current.schedules.find(
            (s) => s.id === issue.schedule_id,
          );
          const period = schedule?.periods.find(
            (p) => p.id === issue.period_id,
          );
          let message = S[`validation_${issue.code}`] ?? S.fixSchedule;
          if (issue.conflict) {
            const other = draftRef.current.schedules.find(
              (s) => s.id === issue.conflict?.schedule_id,
            );
            message = fmt(message, {
              other: `${other?.name ?? ""} · ${issue.conflict.start}–${issue.conflict.end}`,
            });
          }
          return `${[schedule?.name, period ? `${period.start}–${period.end}` : ""].filter(Boolean).join(" · ")}${schedule ? ": " : ""}${message}`;
        })
        .join("\n"),
    );
  }
  function current() {
    const prepared = prepareSettings(snapshotDraft());
    if (!prepared.settings)
      throw new Error(
        prepared.errors
          .map((e) => S[`validation_${e.code}`] ?? S.fixSchedule)
          .join("\n"),
      );
    return prepared.settings;
  }
  async function save(event: FormEvent) {
    event.preventDefault();
    if (busy || saving) return;
    const prepared = prepareSettings(snapshotDraft());
    if (!prepared.settings) {
      displayErrors(prepared.errors);
      return;
    }
    const version = revision.current;
    const editedKeys = [...touched.current];
    setSaving(true);
    setFailure("");
    setErrors([]);
    setSuccess(false);
    try {
      const latest = await invoke<Settings>("get_settings");
      const settings = { ...latest };
      for (const key of editedKeys)
        Object.assign(settings, { [key]: prepared.settings[key] });
      await invoke("save_settings", { settings });
      const updated = await refreshSettings();
      if (revision.current === version) {
        touched.current.clear();
        setDraft(structuredClone(updated));
        setDirty(false);
        setSuccess(true);
      }
    } catch (cause) {
      const error = cause as {
        kind?: string;
        errors?: ValidationError[];
        message?: string;
      };
      if (error.kind === "validation" && error.errors)
        displayErrors(error.errors);
      else
        setFailure(
          error.kind === "resource"
            ? S.localVideoUnavailable
            : `${S.saveFailed} ${error.message ?? String(cause)}`,
        );
    } finally {
      setSaving(false);
    }
  }
  function fieldError(field: string) {
    const issue = errors.find((e) => !e.schedule_id && e.field === field);
    return issue ? (S[`validation_${issue.code}`] ?? S.fixSchedule) : undefined;
  }
  const items = [
    {
      id: "timer" as const,
      label: S.timerSection,
      description: S.timerDescription,
      icon: <ClockIcon />,
    },
    {
      id: "schedules" as const,
      label: S.scheduleTitle,
      description: S.scheduleDescription,
      icon: <CalendarBlankIcon />,
    },
    {
      id: "reminders" as const,
      label: S.remindersSection,
      description: S.remindersDescription,
      icon: <BellIcon />,
    },
    {
      id: "general" as const,
      label: S.commonSettings,
      description: S.generalDescription,
      icon: <GearSixIcon />,
    },
  ];
  return (
    <DatesProvider
      settings={{
        locale: lang === "zh" ? "zh-cn" : "en",
        firstDayOfWeek: 1,
        weekendDays: [0, 6],
      }}
    >
      <form
        ref={form}
        onSubmit={(event) => void save(event)}
        noValidate
        className="settings-shell"
        id="settingsForm"
      >
        <aside className="settings-sidebar">
          <div
            className="sidebar-brand"
            onMouseDown={dragWindow}
            data-tauri-drag-region
          >
            <ClockIcon size={29} />
            <Text fw={650}>Sit Break</Text>
          </div>
          <nav aria-label={S.settingsTitle}>
            {items.map((item) => (
              <Tooltip
                key={item.id}
                label={item.label}
                position="right"
                openDelay={500}
              >
                <NavLink
                  component="button"
                  type="button"
                  label={item.label}
                  aria-label={item.label}
                  leftSection={item.icon}
                  active={section === item.id}
                  onClick={() => navigate(item.id)}
                  aria-current={section === item.id ? "page" : undefined}
                  data-section={item.id}
                />
              </Tooltip>
            ))}
          </nav>
          <div className="sidebar-appearance">
            <AppearancePicker
              value={draft.appearance}
              onChange={(value) => change("appearance", value)}
              S={S}
            />
          </div>
        </aside>
        <main className="settings-main">
          <ActionIcon
            className="settings-close"
            aria-label={S.closeSettings}
            onClick={() =>
              void invoke("hide_settings").catch(() =>
                setFailure(S.operationFailed),
              )
            }
          >
            <XIcon size={21} />
          </ActionIcon>
          <ScrollArea
            className="settings-scroll"
            viewportRef={viewport}
            type="auto"
            offsetScrollbars
            scrollbarSize={7}
          >
            {items.map((item) => (
              <section
                key={item.id}
                className={`settings-view view-${item.id}`}
                hidden={section !== item.id}
                aria-label={item.label}
              >
                <header
                  className="settings-heading"
                  onMouseDown={dragWindow}
                  data-tauri-drag-region
                >
                  <div>
                    <Title order={1} tabIndex={-1} id={`heading-${item.id}`}>
                      {item.label}
                    </Title>
                    <Text size="sm" c="dimmed">
                      {item.description}
                    </Text>
                  </div>
                  {item.id === "schedules" && (
                    <Button
                      id="previewSchedule"
                      variant="default"
                      size="compact-sm"
                      onClick={() => setPreview(true)}
                    >
                      {S.previewSchedule}
                    </Button>
                  )}
                </header>
                {item.id === "timer" && (
                  <>
                    <div className="durations">
                      <div
                        data-error-key={errorKey({
                          schedule_id: null,
                          period_id: null,
                          field: "work_minutes",
                        })}
                      >
                        <DurationControl
                          id="workMin"
                          label={S.workLength}
                          value={draft.work_minutes}
                          max={240}
                          presets={[15, 30, 45, 60]}
                          S={S}
                          error={fieldError("work_minutes")}
                          onChange={(value) => change("work_minutes", value)}
                        />
                      </div>
                      <div
                        data-error-key={errorKey({
                          schedule_id: null,
                          period_id: null,
                          field: "break_minutes",
                        })}
                      >
                        <DurationControl
                          id="breakMin"
                          label={S.breakLen}
                          value={draft.break_minutes}
                          max={60}
                          presets={[3, 5, 10]}
                          S={S}
                          error={fieldError("break_minutes")}
                          onChange={(value) => change("break_minutes", value)}
                        />
                      </div>
                    </div>
                    <Text size="sm" c="dimmed" className="simple-summary">
                      {fmt(S.simpleSummary, {
                        work:
                          draft.work_minutes === "" ? "—" : draft.work_minutes,
                        rest:
                          draft.break_minutes === ""
                            ? "—"
                            : draft.break_minutes,
                      })}
                    </Text>
                    {draft.use_schedule && (
                      <Alert
                        className="simple-inactive"
                        variant="light"
                        color="plum"
                      >
                        {S.simpleInactive}
                      </Alert>
                    )}
                    <div className="timer-preferences">
                      <PreferenceRow
                        icon={<BellIcon />}
                        id="swSound"
                        title={S.sound}
                        description={S.soundDescription}
                        checked={draft.sound}
                        onChange={(value) => change("sound", value)}
                      />
                      <PreferenceRow
                        icon={<ArrowCounterClockwiseIcon />}
                        id="swIdleReset"
                        title={S.idleReset}
                        description={S.idleDescription}
                        checked={draft.idle_reset}
                        onChange={(value) => change("idle_reset", value)}
                      />
                      <PreferenceRow
                        icon={<CalendarBlankIcon />}
                        id="swSchedule"
                        title={S.useSchedule}
                        description={S.scheduleOptionalDescription}
                        checked={draft.use_schedule}
                        onChange={(value) => change("use_schedule", value)}
                      >
                        <Anchor
                          component="button"
                          type="button"
                          size="xs"
                          className="edit-schedules"
                          onClick={() => navigate("schedules")}
                        >
                          {S.editSchedule}
                        </Anchor>
                      </PreferenceRow>
                    </div>
                    <div className="next-reminder">
                      <FileVideoIcon size={23} />
                      <Text size="xs" c="dimmed">
                        {S.nextReminder}
                      </Text>
                    </div>
                  </>
                )}
                {item.id === "schedules" && (
                  <ScheduleEditor
                    draft={draft}
                    onChange={(value) => change("schedules", value)}
                    S={S}
                    errors={errors}
                    registry={clocks.current}
                    onEditing={() => mark("schedules")}
                    onBusy={onBusy}
                    importing={busy}
                  />
                )}
                {item.id === "reminders" && (
                  <Stack gap={22} id="tipsList">
                    <Text size="sm" c="dimmed">
                      {S.tipsHint} · {S.tipsDescription}
                    </Text>
                    {draft.tips.map((tip, index) => (
                      <article className="reminder-entry" key={tip.id}>
                        <Group justify="space-between" mb={10}>
                          <Text fw={600} size="sm">
                            {fmt(S.reminderEntry, { n: index + 1 })}
                          </Text>
                          <ActionIcon
                            aria-label={fmt(S.removeReminderEntry, {
                              n: index + 1,
                            })}
                            color="red"
                            disabled={busy}
                            onClick={() =>
                              change(
                                "tips",
                                draft.tips.filter(
                                  (entry) => entry.id !== tip.id,
                                ),
                              )
                            }
                          >
                            <TrashIcon size={17} />
                          </ActionIcon>
                        </Group>
                        <Textarea
                          label={S.tipsHeading}
                          value={tip.text}
                          autosize
                          minRows={3}
                          maxRows={10}
                          error={fieldError(`tips.${index}.text`)}
                          data-field={`tips.${index}.text`}
                          data-error-key={errorKey({
                            schedule_id: null,
                            period_id: null,
                            field: `tips.${index}.text`,
                          })}
                          onChange={(e) =>
                            change(
                              "tips",
                              draft.tips.map((entry) =>
                                entry.id === tip.id
                                  ? { ...entry, text: e.currentTarget.value }
                                  : entry,
                              ),
                            )
                          }
                        />
                        <GuidancePicker
                          S={S}
                          value={tip.guidance}
                          onBusy={onBusy}
                          error={fieldError(`tips.${index}.guidance`)}
                          field={`tips.${index}.guidance`}
                          onChange={(guidance) =>
                            change(
                              "tips",
                              draft.tips.map((entry) =>
                                entry.id === tip.id
                                  ? { ...entry, guidance }
                                  : entry,
                              ),
                            )
                          }
                        />
                      </article>
                    ))}
                    <Button
                      variant="light"
                      leftSection={<PlusIcon size={17} />}
                      id="addReminder"
                      onClick={() => {
                        change("tips", [
                          ...draft.tips,
                          { id: crypto.randomUUID(), text: "" },
                        ]);
                        requestAnimationFrame(() => {
                          const fields =
                            document.querySelectorAll<HTMLTextAreaElement>(
                              ".reminder-entry textarea",
                            );
                          fields[fields.length - 1]?.focus();
                        });
                      }}
                    >
                      {plainLabel(S.addReminder)}
                    </Button>
                  </Stack>
                )}
                {item.id === "general" && (
                  <div className="general-settings">
                    <div className="general-row">
                      <div>
                        <Text fw={600} size="sm">
                          {S.appearance}
                        </Text>
                        <Text size="xs" c="dimmed" mt={4}>
                          {S.appearanceDescription}
                        </Text>
                      </div>
                      <SegmentedControl
                        value={draft.appearance}
                        aria-label={S.appearance}
                        onChange={(value) =>
                          change(
                            "appearance",
                            value as SettingsDraft["appearance"],
                          )
                        }
                        data={[
                          { value: "system", label: S.langSys },
                          { value: "light", label: S.lightTheme },
                          { value: "dark", label: S.darkTheme },
                        ]}
                      />
                    </div>
                    <div className="general-row">
                      <Group gap={10}>
                        <GlobeIcon size={20} />
                        <Text fw={600} size="sm">
                          {S.language}
                        </Text>
                      </Group>
                      <Select
                        id="language"
                        aria-label={S.language}
                        value={draft.language}
                        onChange={(value) =>
                          change("language", value ?? "system")
                        }
                        data={[
                          { value: "system", label: S.langSys },
                          { value: "zh", label: "简体中文" },
                          { value: "en", label: "English" },
                        ]}
                      />
                    </div>
                    <PreferenceRow
                      icon={<PowerIcon />}
                      title={S.autostart}
                      id="swAuto"
                      description={S.autostartDescription}
                      checked={draft.autostart}
                      onChange={(value) => change("autostart", value)}
                    />
                    {platform !== "macos" && (
                      <>
                        <PreferenceRow
                          icon={<MonitorIcon />}
                          title={S.showWidget}
                          id="swWidget"
                          checked={draft.show_widget}
                          onChange={(value) => change("show_widget", value)}
                        />
                        <div className="general-row">
                          <Text fw={600} size="sm">
                            {S.ballSize}
                          </Text>
                          <NumberInput
                            id="widgetSize"
                            aria-label={S.ballSize}
                            value={draft.widget_size}
                            min={40}
                            max={140}
                            suffix=" px"
                            error={fieldError("widget_size")}
                            data-error-key={errorKey({
                              schedule_id: null,
                              period_id: null,
                              field: "widget_size",
                            })}
                            onChange={(value) => change("widget_size", value)}
                          />
                        </div>
                      </>
                    )}
                  </div>
                )}
              </section>
            ))}
          </ScrollArea>
          <footer className="settings-footer">
            {failure && (
              <Alert
                color="red"
                variant="light"
                className="save-error"
                id="saveError"
                role="alert"
              >
                {failure}
              </Alert>
            )}
            <div className="footer-actions">
              <Text
                size="xs"
                c="dimmed"
                id="saveStatus"
                role="status"
                aria-live="polite"
              >
                {success ? (
                  <span className="saved-status">
                    <CheckIcon size={15} />
                    {S.saved}
                  </span>
                ) : busy ? (
                  S.importingWait
                ) : dirty ? (
                  S.unsaved
                ) : (
                  S.localSettings
                )}
              </Text>
              <Button
                type="submit"
                id="btnSave"
                loading={saving}
                disabled={busy}
                aria-label={S.saveSettings}
              >
                {S.saveSettings}
              </Button>
            </div>
          </footer>
        </main>
      </form>
      <SchedulePreview
        opened={preview}
        onClose={() => setPreview(false)}
        getDraft={current}
        S={S}
      />
    </DatesProvider>
  );
}
