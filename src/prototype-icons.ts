// 使用 Tabler 官方图标，避免用字符代替操作图标。仅供开发原型使用。
import eye from "@tabler/icons/outline/eye.svg?raw";
import clock from "@tabler/icons/outline/clock.svg?raw";
import close from "@tabler/icons/outline/x.svg?raw";
import play from "@tabler/icons/outline/player-play.svg?raw";
import up from "@tabler/icons/outline/chevron-up.svg?raw";
import down from "@tabler/icons/outline/chevron-down.svg?raw";
import plus from "@tabler/icons/outline/plus.svg?raw";
import sun from "@tabler/icons/outline/sun.svg?raw";
import afternoon from "@tabler/icons/outline/sun-low.svg?raw";
import left from "@tabler/icons/outline/arrow-left.svg?raw";
import right from "@tabler/icons/outline/arrow-right.svg?raw";
import below from "@tabler/icons/outline/arrow-down.svg?raw";
import refresh from "@tabler/icons/outline/refresh.svg?raw";
import check from "@tabler/icons/outline/check.svg?raw";
import alert from "@tabler/icons/outline/alert-circle.svg?raw";

const icons = { eye, clock, close, play, up, down, plus, sun, afternoon, left, right, below, refresh, check, alert };
export const icon = (name: keyof typeof icons) => icons[name].replace("<svg", '<svg aria-hidden="true" focusable="false"');
