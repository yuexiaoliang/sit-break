# Schedules and Breaks

**English** | [简体中文](schedules.md)

## Simple Timer Is the Default

On first launch, Simple timer works without creating schedules. Open Work & breaks, choose Simple timer, and enter work/break lengths or use the presets. Work length excludes the break. Language, sound, launch at login, and reminder content are shared. Auto-reset when away applies only to Simple timer. macOS uses the menu bar; Windows also lets you show, hide, and resize the floating timer.

Choose Scheduled timer and save to follow detailed schedules. Its overview lists enabled schedules, repeat dates, time periods, and default lengths. Open the editor through Edit schedules or the Schedules sidebar page. Switching modes preserves both drafts; saving applies the choice. With no active period, Scheduled timer stays idle and does not fall back to Simple timer. Upgrades preserve the selected mode and configuration; theme changes also preserve drafts.

## Create Schedules and Time Periods

Add a schedule, give it a name, and choose its repeat rule:

- Once uses a date in the computer's local timezone.
- Every week uses selected weekdays, with shortcuts for weekdays or every day.

Each schedule has its own enabled switch. Keep separate weekday and weekend plans if useful. Active periods in enabled schedules must not overlap. If they conflict, saving identifies the relevant schedules, dates or weekdays, and periods; the previous valid settings remain active. Adjacent periods may touch.

Each period has explicit start/end times and default lengths. Work must be an integer from 1–240 minutes, and breaks from 1–60. End times support `24:00`. Split an overnight plan into periods on separate days. The plan, calculated from the period's start, must fit at least one complete round of work and break. A break reminder window may end exactly at the period boundary.

Periods include their start time and exclude their end time. The app schedules rounds from each period’s start and joins the current round and phase when you open it. For a 09:00–12:00 period with 55 minutes of work and 5 minutes of rest per round, opening at 10:20 puts you in round 2. Round 3 is scheduled to start at 11:00. Any time left over that cannot fit a full round stays idle.

## Adjust Individual Rounds

Expand Round adjustments (optional), enter a round number, and override only the fields that differ. Empty lengths inherit the period defaults. Removing an adjustment restores inheritance. Each period numbers its rounds from 1. If the period is full when you select Add round adjustment, a message appears beside the button. If a shorter round can fit, the editor shows the minutes available for work and break. The editor highlights any adjusted rounds that extend past the period as you edit.

For a morning period with defaults of 55 minutes of work and a 5-minute break:

| Round | Work | Break | Break content |
| --- | --- | --- | --- |
| Round 1 | 50 minutes | 10 minutes | Climb stairs |
| Round 2 | Inherit 55 minutes | Inherit 5 minutes | Squats |
| Later rounds | Inherit 55 minutes | Inherit 5 minutes | Use a default reminder |

Break content can describe multiple activities. Line breaks and special characters remain as entered. The whole break shares one countdown; the app neither splits activities nor translates your text. A round can also import one local MP4/M4V video, with or without text. If both text and video are absent, the app randomly selects a complete global reminder entry. The same content remains throughout snoozing, reminding, and resting in that round.

## Preview the Plan

Select Preview schedule beside the page heading to open Draft plan, which shows the draft’s complete rounds and gaps. Switch to Current progress to see the active round and the saved upcoming rounds. It shows the countdown once a break starts. Unsaved changes do not affect the timer. Use `Tab` and the arrow keys to switch views or scroll. Close with the close button, `Esc`, or by clicking outside. Focus returns to the preview button and your edits stay in place.

## Start or Snooze Within the Break Window

A reminder appears at the scheduled break time. It shows how long you have to start and how long the break will last. If a five-minute break is due at 09:55 and you start it at 09:57, it counts down from 5:00 and ends at 10:02. The timer then rejoins the schedule. If round 2 was scheduled to begin at 10:00 with 55 minutes of work, it has 53 minutes of work left. Later reminders keep their scheduled times. A break in progress can finish across a period boundary or midnight.

Snooze hides the reminder for up to five minutes. It appears again if the reminder window is still open. Starting it after snoozing also gives you the full duration. End break closes the break window early and rejoins the schedule at the current time.

Pause reminders mutes and hides reminders while the schedule and countdown keep running. Resume reminders lets you use the current break window if it is still open. Gaps and expired rounds do not send catch-up reminders.

## Save Changes, Reopen, and Wake

Saving new durations or period boundaries applies the updated schedule. A break already in progress keeps its duration, text, and video. Disabling its schedule, removing its period, or switching timer modes ends that scheduled break immediately.

An active break counts time spent while the app is closed or the computer is asleep. When you reopen the app or wake the computer, the app shows any remaining break time and the original break content. If the break has finished, the timer rejoins the schedule at the current time. Reopening also remembers your pause preference and any snooze or early ending in the current reminder window.

Simple timer still counts actual elapsed time. You can pause its countdown, reset work, and enable Auto-reset when away. Computer sleep does not count toward work; a break already in progress counts the elapsed time.

See the [schedule verification record](schedule-verification.md), maintained in Simplified Chinese, for developer checks and native test coverage.
