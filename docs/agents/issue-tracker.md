# 任务追踪：GitHub

本仓库的任务和功能规格保存在 `yuexiaoliang/sit-break` 的 GitHub Issues 中。从仓库根目录运行 `gh` 命令，工具会通过 `origin` 确定目标仓库。在其他目录运行时，添加 `--repo yuexiaoliang/sit-break`。

## 任务操作

对于多行的任务、评论或拉取请求正文，先将完整 Markdown 写入 UTF-8 临时文件，再通过 `--body-file <file>` 传入，以保留换行和原始文本。

- **创建任务**：`gh issue create --title "..." --body-file <file>`。
- **读取任务**：`gh issue view <number> --json number,title,body,labels,comments`。
- **列出任务**：`gh issue list --state open --limit 100 --json number,title,body,labels,comments`；按需添加 `--label` 和 `--state` 筛选条件。需要完整清单时，使用分页或提高数量上限。
- **修改正文**：`gh issue edit <number> --body-file <file>`。
- **发表评论**：`gh issue comment <number> --body-file <file>`。
- **添加或移除标签**：`gh issue edit <number> --add-label "..."` 或 `--remove-label "..."`；标签名称遵循 [分类标签映射](triage-labels.md)。
- **关闭任务**：`gh issue close <number>`。需要说明原因时，先发表评论。

当技能要求**发布到任务追踪系统**时，创建 GitHub Issue。当技能要求**获取相关任务**时，读取对应 Issue 及其评论。

## 父任务与前置依赖

使用 GitHub 原生子任务和任务依赖关系，让它们能在任务追踪界面中直接查看。

- **创建子任务**：`gh issue create --parent <parent-number> --title "..." --body-file <file>`。
- **关联已有子任务**：`gh issue edit <parent-number> --add-sub-issue <child-number>`。
- **添加前置依赖**：`gh issue edit <child-number> --add-blocked-by <blocker-number>`；每个前置任务分别添加。
- **创建时指定前置依赖**：`gh issue create --blocked-by <number>,<number> --title "..." --body-file <file>`。
- **查看未关闭的前置任务数量**：`gh api repos/yuexiaoliang/sit-break/issues/<number> --jq '.issue_dependencies_summary.blocked_by'`。该字段统计未关闭的前置任务；确认依赖数据存在后，才能将零解读为没有阻塞。

如果原生关系不可用，在子任务正文中保留 `Part of #<parent>` 和 `Blocked by: #<number>, #<number>` 这两个固定格式，并在父任务正文中列出子任务清单。所有前置任务都已关闭时，该任务才不再受阻塞。

## 将拉取请求作为需求入口

**PRs as a request surface: no.**

此固定配置字段表示关闭外部拉取请求的需求分类入口。若项目以后需要将外部拉取请求纳入此流程，将值改为 `yes`。

启用后，使用 `gh pr view <number> --comments`、`gh pr diff <number>` 及对应的 `gh pr comment`、`gh pr edit`、`gh pr close` 命令。多行评论和描述修改仍通过 `--body-file <file>` 传入。

GitHub 的 Issue 和拉取请求共用编号。引用类型不明确时，先尝试 `gh pr view <number>`，再尝试 `gh issue view <number>`。

## 探索复杂需求

使用 `/wayfinder` 时，以一个 GitHub Issue 作为探索地图，以子任务记录需要解决的决策问题。

- **地图**：创建带有 `wayfinder:map` 标签的任务，正文包含笔记、已有决策和待澄清事项。
- **子任务**：按上述父子任务操作关联到地图，并添加 `wayfinder:<type>` 标签，类型为 `research`、`prototype`、`grilling` 或 `task`。首次使用该流程时，按需创建这些标签。
- **选择可开始的任务**：通过 `gh api --paginate repos/yuexiaoliang/sit-break/issues/<map-number>/sub_issues` 检查地图的子任务；采用兼容格式时，读取父任务中的清单。从尚未关闭的任务中，选择未分配负责人且没有未关闭前置依赖的任务；有多个候选时，按地图中的顺序选择。
- **领取任务**：开始前执行 `gh issue edit <number> --add-assignee @me`。
- **完成任务**：通过 `gh issue comment <number> --body-file <answer-file>` 发布结论，关闭任务，再向地图的“已有决策”部分追加简短结论和任务链接，保留其余内容。
