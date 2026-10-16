import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  createTheme,
  MantineProvider,
  type MantineColorsTuple,
} from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { IconContext } from "@phosphor-icons/react";
import { useSettings } from "./runtime";
import type { Appearance } from "./model";
import "@mantine/core/styles.css";
import "@mantine/dates/styles.css";
import "./app.css";

const plum: MantineColorsTuple = [
  "#f6f3fa",
  "#ece5f4",
  "#d9cde8",
  "#c5b0e2",
  "#af98cc",
  "#927aaf",
  "#74618f",
  "#66517f",
  "#54436c",
  "#403050",
];
const theme = createTheme({
  primaryColor: "plum",
  colors: { plum },
  primaryShade: { light: 6, dark: 3 },
  autoContrast: true,
  defaultRadius: 8,
  fontFamily:
    '-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif',
  fontSizes: { xs: "12px", sm: "14px", md: "15px", lg: "17px", xl: "20px" },
  respectReducedMotion: true,
  focusRing: "auto",
  cursorType: "pointer",
  components: {
    Button: {
      defaultProps: {
        size: "sm",
        radius: 8,
        variant: "filled",
        autoContrast: true,
      },
    },
    ActionIcon: { defaultProps: { variant: "subtle", size: "md" } },
    Switch: { defaultProps: { size: "md", withThumbIndicator: false } },
    TextInput: { defaultProps: { size: "sm" } },
    NumberInput: {
      defaultProps: {
        size: "sm",
        clampBehavior: "none",
        allowNegative: false,
        allowDecimal: false,
      },
    },
    Select: {
      defaultProps: {
        size: "sm",
        allowDeselect: false,
        checkIconPosition: "right",
        comboboxProps: {
          transitionProps: { transition: "fade", duration: 160 },
        },
      },
    },
    Modal: {
      defaultProps: {
        centered: true,
        radius: 14,
        returnFocus: true,
        transitionProps: { transition: "pop", duration: 180 },
        overlayProps: { backgroundOpacity: 0.32, blur: 2 },
      },
    },
    Tooltip: {
      defaultProps: {
        openDelay: 400,
        withArrow: false,
        transitionProps: { duration: 120 },
      },
    },
  },
});
const PreviewTheme = createContext<(value: Appearance | null) => void>(
  () => {},
);
export function usePreviewTheme() {
  return useContext(PreviewTheme);
}
export function AppTheme({ children }: { children: ReactNode }) {
  const settings = useSettings();
  const [preview, setPreview] = useState<Appearance | null>(null);
  const prefersDark = useMediaQuery("(prefers-color-scheme: dark)", false, {
    getInitialValueInEffect: false,
  });
  const mode = preview ?? settings.appearance;
  const scheme = mode === "system" ? (prefersDark ? "dark" : "light") : mode;
  useEffect(() => {
    document.documentElement.dataset.appearance = mode;
    localStorage.setItem("sit-break-appearance", settings.appearance);
  }, [settings.language, settings.appearance, mode]);
  const iconStyle = useMemo(
    () => ({ size: 21, weight: "regular" as const }),
    [],
  );
  return (
    <MantineProvider
      theme={theme}
      forceColorScheme={scheme}
      env={import.meta.env.MODE === "test" ? "test" : undefined}
    >
      <IconContext.Provider value={iconStyle}>
        <PreviewTheme.Provider value={setPreview}>
          {children}
        </PreviewTheme.Provider>
      </IconContext.Provider>
    </MantineProvider>
  );
}
