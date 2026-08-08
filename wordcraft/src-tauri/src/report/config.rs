//! 周报的 SMTP 配置。spec §4.2 F13「仅安装时配置文件设置，客户端界面不出现任何相关入口」。
//!
//! **凭据不进仓库，也不进数据库。** 配置文件放在系统的应用配置目录下，由使用者
//! 自己创建。代码里没有任何默认账号密码，缺配置时功能整体关闭而不是报错——
//! 没配周报的用户不该看到任何与周报有关的东西，这正是 spec 要的隐蔽性。

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// 配置文件名。放在 Tauri 的 app config dir 下。
pub const CONFIG_FILE: &str = "report.json";

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SmtpConfig {
    /// SMTP 服务器，如 `smtp.qq.com`
    pub host: String,
    /// 465 为隐式 TLS，587 为 STARTTLS
    #[serde(default = "default_port")]
    pub port: u16,
    /// 发件邮箱
    pub username: String,
    /// 授权码（多数国内邮箱用授权码而非登录密码）
    pub password: String,
    /// 收件人，通常是家长
    pub to: String,
    /// 发件人显示名
    #[serde(default = "default_from_name")]
    pub from_name: String,
}

fn default_port() -> u16 {
    465
}

fn default_from_name() -> String {
    "WordCraft".to_string()
}

/// 配置的载入结果。
///
/// 「没配置」与「配置坏了」必须分开：前者是正常状态（用户没启用周报），
/// 后者是用户想启用但填错了，得让他知道。
#[derive(Debug, PartialEq)]
pub enum Loaded {
    /// 文件不存在——周报未启用
    Disabled,
    Enabled(Box<SmtpConfig>),
    /// 文件存在但不可用，附带原因
    Invalid(String),
}

/// 校验必填项。空字符串等同于没填——JSON 里留 `""` 是常见的「先占位」写法，
/// 拿它去连 SMTP 只会得到一个难懂的服务器错误。
fn validate(c: &SmtpConfig) -> Result<(), String> {
    for (field, value) in [
        ("host", &c.host),
        ("username", &c.username),
        ("password", &c.password),
        ("to", &c.to),
    ] {
        if value.trim().is_empty() {
            return Err(format!("`{field}` 不能为空"));
        }
    }
    if !c.to.contains('@') {
        return Err(format!("收件地址 `{}` 不像邮箱", c.to));
    }
    if !c.username.contains('@') {
        return Err(format!("发件账号 `{}` 不像邮箱", c.username));
    }
    if c.port == 0 {
        return Err("`port` 不能为 0".to_string());
    }
    Ok(())
}

pub fn load_from(path: &Path) -> Loaded {
    if !path.exists() {
        return Loaded::Disabled;
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return Loaded::Invalid(format!("无法读取 {}: {e}", path.display())),
    };
    let cfg: SmtpConfig = match serde_json::from_str(&text) {
        Ok(c) => c,
        Err(e) => return Loaded::Invalid(format!("{} 格式有误: {e}", path.display())),
    };
    match validate(&cfg) {
        Ok(()) => Loaded::Enabled(Box::new(cfg)),
        Err(e) => Loaded::Invalid(format!("{} 配置无效: {e}", path.display())),
    }
}

/// 配置文件的完整路径。
pub fn config_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(CONFIG_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("wordcraft-report-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    const GOOD: &str = r#"{
      "host": "smtp.example.com",
      "port": 465,
      "username": "sender@example.com",
      "password": "auth-code",
      "to": "parent@example.com"
    }"#;

    #[test]
    fn 文件不存在时为未启用而非错误() {
        // 绝大多数用户不会配周报，那是正常状态，不该产生错误日志
        let path = std::env::temp_dir().join("wordcraft-nonexistent-xyz.json");
        assert_eq!(load_from(&path), Loaded::Disabled);
    }

    #[test]
    fn 合法配置载入成功并应用默认值() {
        let path = write_temp("good.json", GOOD);
        match load_from(&path) {
            Loaded::Enabled(c) => {
                assert_eq!(c.host, "smtp.example.com");
                assert_eq!(c.port, 465);
                assert_eq!(c.from_name, "WordCraft", "未填时应有默认发件人名");
            }
            other => panic!("应载入成功，实际 {other:?}"),
        }
    }

    #[test]
    fn 端口可省略默认为隐式tls的465() {
        // 国内邮箱普遍用 465 隐式 TLS，作为默认值能省掉一次配置错误
        let path = write_temp("noport.json", r#"{
          "host": "smtp.qq.com",
          "username": "a@qq.com",
          "password": "x",
          "to": "b@qq.com"
        }"#);
        match load_from(&path) {
            Loaded::Enabled(c) => assert_eq!(c.port, 465),
            other => panic!("应载入成功，实际 {other:?}"),
        }
    }

    #[test]
    fn 空字段被判为无效而不是拿去连服务器() {
        // 留 "" 占位是常见写法，直接拿去连只会得到难懂的服务器错误
        let path = write_temp("empty.json", r#"{
          "host": "smtp.example.com",
          "username": "a@example.com",
          "password": "",
          "to": "b@example.com"
        }"#);
        match load_from(&path) {
            Loaded::Invalid(e) => assert!(e.contains("password"), "应指出是哪个字段: {e}"),
            other => panic!("应判为无效，实际 {other:?}"),
        }
    }

    #[test]
    fn 非邮箱形式的收件地址被拒() {
        let path = write_temp("badto.json", r#"{
          "host": "smtp.example.com",
          "username": "a@example.com",
          "password": "x",
          "to": "parent"
        }"#);
        match load_from(&path) {
            Loaded::Invalid(e) => assert!(e.contains("parent"), "应回显错误值: {e}"),
            other => panic!("应判为无效，实际 {other:?}"),
        }
    }

    #[test]
    fn 损坏的json报明确错误而非静默禁用() {
        // 用户明明配了却不发信，最难排查——必须区分「没配」和「配坏了」
        let path = write_temp("broken.json", "{ not json");
        match load_from(&path) {
            Loaded::Invalid(e) => assert!(e.contains("格式有误"), "{e}"),
            other => panic!("损坏文件不能当作未启用，实际 {other:?}"),
        }
    }

    #[test]
    fn 缺必填字段报错而非用默认值兜底() {
        let path = write_temp("missing.json", r#"{"host": "smtp.example.com"}"#);
        assert!(
            matches!(load_from(&path), Loaded::Invalid(_)),
            "缺 username/password/to 不能默默通过"
        );
    }
}
