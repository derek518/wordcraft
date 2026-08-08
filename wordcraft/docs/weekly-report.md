# 家长周报（F13）

每周日晚 20:00 自动把一周学习情况发到家长邮箱。

**应用界面里没有任何入口，也不会有。** 这是 spec 的明确要求——周报是给家长看的，
孩子不应该在界面上看到它存在。启用方式只有一个：手工放一个配置文件。

## 启用

在应用数据目录下新建 `report.json`：

| 系统 | 路径 |
|---|---|
| Windows | `%APPDATA%\com.wordcraft.app\report.json` |
| macOS | `~/Library/Application Support/com.wordcraft.app/report.json` |

这个目录里已经有 `wordcraft.db`，找到那个文件就找对地方了。

```json
{
  "host": "smtp.qq.com",
  "port": 465,
  "username": "发件邮箱@qq.com",
  "password": "邮箱授权码",
  "to": "家长邮箱@example.com",
  "from_name": "WordCraft"
}
```

| 字段 | 必填 | 说明 |
|---|---|---|
| `host` | 是 | SMTP 服务器地址 |
| `port` | 否，默认 465 | 465 为隐式 TLS，587 为 STARTTLS |
| `username` | 是 | 发件邮箱，同时用作 SMTP 登录名 |
| `password` | 是 | **授权码，不是邮箱登录密码** |
| `to` | 是 | 收件人 |
| `from_name` | 否，默认 `WordCraft` | 发件人显示名 |

### 关于授权码

国内邮箱（QQ、163、126）不接受登录密码做 SMTP 认证，必须先在邮箱网页端开启
「SMTP 服务」并生成一串授权码。填错会得到「发送被拒」，那基本就是这里的问题。

常见服务商：

| 服务商 | host | port |
|---|---|---|
| QQ 邮箱 | `smtp.qq.com` | 465 |
| 163 邮箱 | `smtp.163.com` | 465 |
| Gmail | `smtp.gmail.com` | 587 |
| Outlook | `smtp.office365.com` | 587 |

## 验证配置

改完配置后不必等到周日。设置环境变量启动一次，会立刻试发一封：

```bash
WORDCRAFT_REPORT_TEST=1 open -a WordCraft
```

Windows PowerShell：

```powershell
$env:WORDCRAFT_REPORT_TEST=1; & "$env:LOCALAPPDATA\WordCraft\WordCraft.exe"
```

结果写在应用日志里（成功会记 `试发成功`，失败会记具体原因）。用环境变量而不是
界面按钮，同样是为了不在界面上留痕迹。

## 发送时机

- 周日 20:00 之后，发送**本周**（周一至周日）的报告
- 如果那天应用没开，下次启动时会把上一个完整周补发出去——错过时机不等于不发
- 同一周只发一次；发送失败则保持未发状态，下一小时重试

首次启用时不会立刻发上一周的报告（那一周多半是空的），而是记下基线，
从下一个完整周开始。

## 报告内容

纯文本，含完成时段数与完成率、新学词数、复习次数、正确率、词汇量估算、
连续天数，以及最顽固的 10 个词（按遗忘次数排序）。

措辞对家长是客观的，不做鼓励性修饰——把进展说得比实际好，会让家长在孩子
真正卡住的时候毫无察觉。

## 不发送的情况

| 情况 | 行为 |
|---|---|
| 没有 `report.json` | 功能整体关闭，不记错误 |
| 配置字段缺失或为空 | 记 warn 日志，不发送 |
| JSON 格式损坏 | 记 warn 日志，**不当作未启用**（配了却不发最难排查） |
| 发送失败 | 记 warn 日志，下一小时重试 |

## 隐私

配置文件里有邮箱授权码。它只存在于本机的应用数据目录，不进数据库、不进仓库、
不随应用分发。仓库的 `.gitignore` 已排除 `report.json`，防止误提交。
