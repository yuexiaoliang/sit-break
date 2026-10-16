import { useEffect, useRef, type ReactNode } from "react";
import {
  ActionIcon,
  Button,
  Group,
  NumberInput,
  SegmentedControl,
  Stack,
  Switch,
  Text,
  Tooltip,
  type NumberInputHandlers,
} from "@mantine/core";
import { TimePicker } from "@mantine/dates";
import {
  DesktopIcon,
  MinusIcon,
  MoonIcon,
  PlusIcon,
  SunIcon,
} from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Appearance, NumericDraft } from "./model";

export type Strings = Record<string, string>;
export function dragWindow(event: React.MouseEvent) {
  if (
    event.button !== 0 ||
    (event.target as HTMLElement).closest(
      "button,input,select,textarea,a,[role=button]",
    )
  )
    return;
  event.preventDefault();
  void getCurrentWindow().startDragging().catch(console.error);
}
export function AppearancePicker({
  value,
  onChange,
  S,
}: {
  value: Appearance;
  onChange: (value: Appearance) => void;
  S: Strings;
}) {
  return (
    <SegmentedControl
      className="appearance-picker"
      value={value}
      onChange={(next) => onChange(next as Appearance)}
      aria-label={S.appearance}
      data={[
        {
          value: "light",
          label: (
            <Tooltip label={S.lightTheme}>
              <span aria-label={S.lightTheme}>
                <SunIcon size={19} />
              </span>
            </Tooltip>
          ),
        },
        {
          value: "system",
          label: (
            <Tooltip label={S.langSys}>
              <span aria-label={S.langSys}>
                <DesktopIcon size={19} />
              </span>
            </Tooltip>
          ),
        },
        {
          value: "dark",
          label: (
            <Tooltip label={S.darkTheme}>
              <span aria-label={S.darkTheme}>
                <MoonIcon size={19} />
              </span>
            </Tooltip>
          ),
        },
      ]}
    />
  );
}
export function PreferenceRow({
  icon,
  title,
  description,
  checked,
  onChange,
  id,
  children,
}: {
  icon: ReactNode;
  title: string;
  description?: ReactNode;
  checked?: boolean;
  onChange?: (value: boolean) => void;
  id?: string;
  children?: ReactNode;
}) {
  return (
    <div className="preference-row">
      <span className="preference-icon" aria-hidden="true">
        {icon}
      </span>
      <div className="preference-copy">
        <Text component="label" htmlFor={id} fw={600}>
          {title}
        </Text>
        {description && (
          <Text size="xs" c="dimmed" className="preference-description">
            {description}
          </Text>
        )}
        {children}
      </div>
      {onChange && (
        <Switch
          id={id}
          className="preference-switch"
          aria-label={title}
          checked={checked}
          onChange={(e) => onChange(e.currentTarget.checked)}
        />
      )}
    </div>
  );
}
export function DurationControl({
  id,
  label,
  value,
  onChange,
  max,
  presets,
  S,
  error,
}: {
  id: string;
  label: string;
  value: NumericDraft;
  onChange: (value: NumericDraft) => void;
  max: number;
  presets: number[];
  S: Strings;
  error?: string;
}) {
  const handlers = useRef<NumberInputHandlers | undefined>(undefined);
  return (
    <div className="duration-control" data-duration={id}>
      <Text component="label" htmlFor={id} fw={600} className="duration-label">
        {label}
      </Text>
      <div className="duration-value">
        <NumberInput
          id={id}
          value={value}
          onChange={onChange}
          min={1}
          max={max}
          hideControls
          handlersRef={handlers}
          variant="unstyled"
          className="duration-input"
          aria-label={label}
          error={error}
          data-field={id === "workMin" ? "work_minutes" : "break_minutes"}
        />
        <Text size="sm" c="dimmed" className="duration-unit">
          {S.minutesUnit}
        </Text>
        <Stack gap={4} className="duration-stepper">
          <ActionIcon
            variant="default"
            size={28}
            aria-label={`${S.increase} ${label}`}
            onClick={() => handlers.current?.increment()}
          >
            <PlusIcon size={15} />
          </ActionIcon>
          <ActionIcon
            variant="default"
            size={28}
            aria-label={`${S.decrease} ${label}`}
            onClick={() => handlers.current?.decrement()}
          >
            <MinusIcon size={15} />
          </ActionIcon>
        </Stack>
      </div>
      <Group gap={6} wrap="nowrap" className="duration-presets">
        {presets.map((n) => (
          <Button
            key={n}
            variant={Number(value) === n && value !== "" ? "filled" : "default"}
            size="compact-sm"
            aria-pressed={Number(value) === n && value !== ""}
            data-minutes={n}
            onClick={() => onChange(n)}
          >
            {n}
          </Button>
        ))}
      </Group>
    </div>
  );
}
export type ClockRegistry = Map<string, () => string>;
/** Mantine 负责分段输入，业务层允许结束时间 24:00，并读取未完成的片段来阻止静默保存旧值。 */
export function ClockField({
  id,
  label,
  value,
  onChange,
  onEditing,
  registry,
  S,
  error,
  field,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  onEditing: () => void;
  registry: ClockRegistry;
  S: Strings;
  error?: string;
  field: string;
}) {
  const hours = useRef<HTMLInputElement>(null),
    minutes = useRef<HTMLInputElement>(null);
  useEffect(() => {
    registry.set(
      id,
      () =>
        `${hours.current?.value ? hours.current.value.padStart(2, "0") : ""}:${minutes.current?.value ? minutes.current.value.padStart(2, "0") : ""}`,
    );
    return () => {
      registry.delete(id);
    };
  }, [id, registry]);
  return (
    <TimePicker
      type="duration"
      label={label}
      value={value}
      onChange={onChange}
      error={error}
      hoursRef={hours}
      minutesRef={minutes}
      hoursInputLabel={`${label} · ${S.hours}`}
      minutesInputLabel={`${label} · ${S.minutes}`}
      hoursInputProps={{ "data-field": field, onInput: onEditing }}
      minutesInputProps={{ onInput: onEditing }}
      withDropdown={false}
      className="clock-field"
    />
  );
}
export function plainLabel(text: string) {
  return text.replace(/^[＋+]\s*/, "");
}
