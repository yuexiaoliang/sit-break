# Schedules and Breaks

**English** | [简体中文](schedules.md)

## Simple Timer Is the Default

On first launch, Simple timer works without creating schedules. Open Work & breaks, choose Simple timer, and enter work/break lengths or use the presets. Work length excludes the break. Language, sound, launch at login, auto-reset when away, and reminder content are shared. macOS uses the menu bar; Windows also lets you show, hide, and resize the floating timer.

Choose Scheduled timer and save to follow detailed schedules. Its overview lists enabled schedules, repeat dates, time periods, and default lengths. Open the editor through Edit schedules or the Schedules sidebar page. Switching modes preserves both drafts; saving applies the choice. With no active period, Scheduled timer stays idle and does not fall back to Simple timer. Upgrades preserve the selected mode and configuration; theme changes also preserve drafts.

## Create Schedules and Time Periods

Add a schedule, give it a name, and choose its repeat rule:

- Once uses a date in the computer's local timezone.
- Every week uses selected weekdays, with shortcuts for weekdays or every day.

Each schedule has its own enabled switch. Keep separate weekday and weekend plans if useful. Active periods in enabled schedules must not overlap. If they conflict, saving identifies the relevant schedules, dates or weekdays, and periods; the previous valid settings remain active. Adjacent periods may touch.

Each period has explicit start/end times and default lengths. Work must be an integer from 1–240 minutes, and breaks from 1–60. End times support `24:00`. Split an overnight plan into periods on separate days.

Periods include their start time and exclude their end time. When first launching or entering a new period, the app starts its first complete round even if the period has already begun. It does not infer completed rounds. Reopening during the same period on the same day follows the recovery rules below.

## Adjust Individual Rounds

Expand Round adjustments (optional), enter a round number, and override only the fields that differ. Empty lengths inherit the period defaults. Removing an adjustment restores inheritance. Each period numbers its rounds from 1.

For a morning period with defaults of 55 minutes of work and a 5-minute break:

| Round | Work | Break | Break content |
| --- | --- | --- | --- |
| Round 1 | 50 minutes | 10 minutes | Climb stairs |
| Round 2 | Inherit 55 minutes | Inherit 5 minutes | Squats |
| Later rounds | Inherit 55 minutes | Inherit 5 minutes | Use a default reminder |

Break content can describe multiple activities. Line breaks and special characters remain as entered. The whole break shares one countdown; the app neither splits activities nor translates your text. A round can also import one local MP4/M4V video, with or without text. If both text and video are absent, the app randomly selects a complete global reminder entry. The same content remains throughout snoozing, reminding, and resting in that round.

## Preview the Plan

Select Preview schedule beside the page heading. The dialog shows the draft's name, repeat rule, periods, gaps, and estimated rounds. Previewing neither saves the draft nor starts a timer. Close it with its button, `Esc`, or by clicking outside. Focus returns to the preview button; expanded adjustments and edits remain intact.

The current round uses its already effective lengths and content. Future rounds use the applicable draft configuration. If the plan cannot match current progress, it is shown from the period's start; this does not imply earlier rounds were completed. When paused, estimates explicitly assume resuming now.

Estimated times change with waiting, snoozing, starting or ending a break, and resetting. They are not enforced deadlines. Only the dialog content scrolls, leaving the close control visible. Use `Tab` to move between the close control and content, and arrow keys to scroll.

## Rest When You Are Ready

When work ends, a reminder appears first. Snooze stays in the same round. Each snooze lasts at most 5 minutes, no more than the round's work length, and at least 1 minute. The round number and content remain unchanged.

The full break starts only after you select Take a break. For example, a 10-minute break reminded at 09:50 but started at 09:55 ends at 10:05. The next round follows. Ending a break early completes that round; Reset restarts the current round's work without consuming a round. Manual pause lasts until you resume.

## Lunch Gaps and Period Boundaries

Gaps do not start work or new reminders. At a period's end, a work countdown stops and a pending break reminder closes. A break already in progress can finish.

If a break continues into the next period, complete it first, then start the first round of the period active at that time. Without an active period, stay idle. Unfinished rounds from the previous period are not caught up.

With Auto-reset when away enabled, the threshold is the current round's break length. Being away long enough during work resets that round. Being away long enough after a reminder counts as having rested and advances the round. A continuous absence does not skip several rounds. It records an absence from the computer, not completion of any activity described in the break content.

## Edit While Running

Changing default lengths, round lengths, or content preserves the current round's effective snapshot. Later rounds use the updated configuration. Adjustments to a round that has already started or passed take effect the next time that round runs. Saving shared preferences or using global duration shortcuts does not rewrite the current scheduled round.

Disabling the active schedule or moving its boundaries so that the current time no longer applies stops work or closes a pending reminder immediately. A break already started finishes first, then follows the latest mode or stays idle. Switching modes or disabling and reenabling scheduled timing starts a fresh sequence. Manual pause remains in effect.

## Quit and Reopen

Scheduled timer saves progress on a normal exit. Reopening on the same day, in the same enabled schedule and active period, restores the round, work/reminder/break phase, remaining time, and that round's effective lengths and content. It also preserves manual pause. Time while the app is closed does not count toward work, breaks, or auto-reset thresholds, and does not catch up rounds.

Stale progress is ignored after changing dates or periods, or when the original schedule is disabled, deleted, or no longer active. Start the first round of the currently applicable period, or stay idle without one. An existing manual pause still needs manual resuming.

Switching modes or disabling and reenabling scheduled timing still means starting fresh; quitting and reopening does not undo that choice. If a break was already started when switching, finish it first, then continue with the latest choice. Scheduled timer starts from the first round of the period active then.

## Computer Sleep While the App Is Running

Sleep does not count as work. Waking does not deduct sleep time from work or replay missed reminders. With Auto-reset when away enabled, sleep counts as absence. The current break length determines whether work should reset or a reminder should count as rested. A continuous absence does not skip several rounds.

A break already started does include actual elapsed sleep time. If time remains, continue it after waking. If it has finished, complete only that round, then choose the next round in the same period, the first round of a new period, or idle. Do not catch up several rounds. Period boundaries still apply; expired work and pending reminders do not resume.

During manual pause, sleep does not advance the countdown or trigger auto-reset. Period boundaries still apply, and an already started break remains until manually resumed. Quitting freezes remaining time, so reopening differs from waking while the app stayed running.

Developer validation results and remaining native checks are recorded in the [schedule verification record](schedule-verification.md), maintained in Simplified Chinese.
