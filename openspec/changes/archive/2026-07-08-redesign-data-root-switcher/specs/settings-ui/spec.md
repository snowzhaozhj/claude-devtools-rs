## MODIFIED Requirements

### Requirement: General Section 展示

General section SHALL 展示当前配置值。MVP 阶段 SHALL 至少展示 theme 设置与数据根目录设置。数据根目录设置 SHALL 显示当前 `general.claudeRootPath`；当值为 `null` 时，UI SHALL 明确展示正在使用默认数据根目录。数据根目录的展示文案 SHALL 使用中性表述（“数据目录” / “数据根目录”），不硬绑定特定来源名称。

数据根目录设置 SHALL 使用轻量 source switcher：当前目录 SHALL 只显示一次，路径作为主文本，右侧以低权重文案标记“默认”或“自定义”；历史快速切换列表的候选来自 `general.recentRoots`（详 [[configuration-management]]），但 SHALL 过滤掉当前目录。过滤后仍有候选时，UI SHALL 渲染“最近”列表，用户 SHALL 能从该列表一键切换到其它历史数据根；过滤后无候选时，UI SHALL 隐藏“最近”列表而不是渲染空白控件。快速切换 SHALL 通过既有 `claudeRootPath` 更新路径落地，切换成功后 SHALL 反映新的当前数据根。

手动路径输入 SHALL 默认收起；用户点击“输入路径”后，原按钮行 SHALL 原地替换为路径输入行，不插入新的中间块。输入行 SHALL 支持应用与取消；应用失败时 SHALL 保持输入行可见并在输入附近显示错误，取消时 SHALL 不修改配置并恢复按钮行。

当存在已打开的 root-scoped tab 时，数据目录区 SHALL 显示轻量提示，说明切换成功后会关闭当前会话 tab 并回到工作台；不存在此类 tab 时 SHALL NOT 显示该提示。

#### Scenario: 展示当前 theme
- **WHEN** General section 渲染
- **THEN** SHALL 显示当前 theme 值（dark/light/system）

#### Scenario: 展示默认数据根目录
- **WHEN** General section 渲染且 `general.claudeRootPath = null`
- **THEN** SHALL 显示默认数据根目录路径（如 `~/.claude`）
- **AND** SHALL 以低权重文案标记该路径为“默认”
- **AND** SHALL NOT 把“默认”作为路径文本前缀拼接展示

#### Scenario: 展示自定义数据根目录
- **WHEN** General section 渲染且 `general.claudeRootPath = "/data/claude-alt"`
- **THEN** SHALL 显示 `/data/claude-alt`
- **AND** SHALL 以低权重文案标记该路径为“自定义”

#### Scenario: 从历史快速切换数据根
- **WHEN** `general.recentRoots` 含至少一个不同于当前数据根的历史路径
- **AND** 用户在“最近”列表中选择其中一项
- **THEN** 系统 SHALL 通过 `claudeRootPath` 更新路径切换到该数据根
- **AND** 切换成功后展示控件 SHALL 反映新的当前数据根

#### Scenario: 最近列表不重复当前目录
- **WHEN** `general.recentRoots` 同时含当前数据根和其它历史路径
- **THEN** “最近”列表 SHALL 只显示其它历史路径
- **AND** 当前数据根 SHALL NOT 在“最近”列表中重复出现

#### Scenario: 无其它历史时隐藏最近列表
- **WHEN** `general.recentRoots` 为空或过滤当前目录后为空
- **THEN** General section SHALL 隐藏“最近”列表
- **AND** SHALL 仍支持选择目录、输入路径两条入口

#### Scenario: 输入路径原地展开
- **WHEN** 用户点击“输入路径”
- **THEN** 按钮行 SHALL 原地替换为路径输入行
- **AND** 当前目录展示行 SHALL 保持位置不变
- **AND** “最近”列表 SHALL NOT 因输入行展开而发生下移跳变

#### Scenario: 输入路径应用成功
- **WHEN** 用户在输入路径行输入合法路径并应用
- **THEN** UI SHALL 调用 `update_config("general", { claudeRootPath: <path> })`
- **AND** 成功后 SHALL 收起输入行并显示新的当前数据根

#### Scenario: 输入路径应用失败
- **WHEN** 用户在输入路径行输入非法路径并应用
- **AND** 后端返回 validation error
- **THEN** UI SHALL 保持输入行可见
- **AND** SHALL 在输入附近显示错误提示
- **AND** SHALL NOT 关闭当前 tab 或刷新工作台上下文

#### Scenario: 清空 Claude root 恢复默认
- **WHEN** 用户点击恢复默认控件
- **THEN** UI SHALL 调用 `update_config("general", { claudeRootPath: null })`
- **AND** 成功后 UI SHALL 显示默认数据根目录状态

#### Scenario: 切换后果提示按需显示
- **WHEN** 存在已打开的 session 或 memory tab
- **THEN** 数据目录区 SHALL 显示切换成功后会关闭当前会话 tab 并回到工作台的提示
- **WHEN** 不存在已打开的 session 或 memory tab
- **THEN** 数据目录区 SHALL NOT 显示该提示
