import { describe, expect, it } from "vitest";
import {
  appearance,
  clockMinutes,
  newPeriod,
  prepareSettings,
  quietGaps,
} from "../src/model";
import { settings } from "./fixtures";
describe("可编辑设置与后端数据的边界", () => {
  it("只允许结束时间为24:00，拒绝未完成片段和非法分钟", () => {
    expect(clockMinutes("24:00")).toBe(1440);
    expect(clockMinutes("24:00", false)).toBeUndefined();
    for (const value of [":00", "09:", "24:01", "23:60", "9:00"])
      expect(clockMinutes(value)).toBeUndefined();
    expect(prepareSettings(settings()).errors).toEqual([]);
  });
  it("不把清空的必填数字变成0；可选轮次时长空值继续继承", () => {
    const draft = settings();
    draft.schedules[0].periods[0].overrides = [
      { round: "2", work_minutes: "", break_minutes: "" },
    ];
    const prepared = prepareSettings(draft);
    expect(prepared.settings?.schedules[0].periods[0].overrides).toEqual([
      { round: 2 },
    ]);
    const invalid = { ...draft, work_minutes: "" };
    expect(prepareSettings(invalid).errors[0].field).toBe("work_minutes");
  });
  it("一天已经排到24:00后新时段留空，不能自动制造重叠区间", () => {
    const draft = settings();
    expect(newPeriod(draft.schedules[0], draft)).toMatchObject({
      start: "",
      end: "",
    });
    expect(
      quietGaps([
        {
          id: "a",
          start: "09:00",
          end: "12:00",
          work_minutes: 45,
          break_minutes: 5,
        },
        {
          id: "b",
          start: "14:00",
          end: "18:00",
          work_minutes: 45,
          break_minutes: 5,
        },
      ]),
    ).toEqual([{ start: "12:00", end: "14:00" }]);
  });
  it("旧版本和未知主题继续跟随系统", () => {
    expect(appearance(undefined)).toBe("system");
    expect(appearance("future")).toBe("system");
  });
});
