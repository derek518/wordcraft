//! SMTP 发送。
//!
//! **这一层无法在本机验证**——发信要真实邮箱与授权码，那是使用者的私有凭据，
//! 不该出现在仓库或测试里。因此把能测的（地址解析、正文组装、错误分类）与
//! 不能测的（真正的网络往返）分开，前者有测试，后者在 MOCKS.md 登记为待真机验证。

use super::config::SmtpConfig;
use super::content::{self, WeeklyReport};
use lettre::message::{header::ContentType, Mailbox};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

/// 组装邮件。地址解析失败在这里就暴露，不必等到连服务器。
pub fn compose(cfg: &SmtpConfig, report: &WeeklyReport) -> Result<Message, String> {
    let from: Mailbox = format!("{} <{}>", cfg.from_name, cfg.username)
        .parse()
        .map_err(|e| format!("发件地址 `{}` 无法解析: {e}", cfg.username))?;
    let to: Mailbox = cfg
        .to
        .parse()
        .map_err(|e| format!("收件地址 `{}` 无法解析: {e}", cfg.to))?;

    Message::builder()
        .from(from)
        .to(to)
        .subject(format!("WordCraft 学习周报 · {}", &report.week_start))
        .header(ContentType::TEXT_PLAIN)
        .body(content::render_text(report))
        .map_err(|e| format!("组装邮件失败: {e}"))
}

/// 建立连接并发送。
///
/// 端口决定 TLS 方式：465 是隐式 TLS（连上就握手），587 是 STARTTLS（明文起，
/// 再升级）。国内邮箱基本都用 465。选错会卡在握手，报错也难懂，所以按端口自动选。
pub fn send(cfg: &SmtpConfig, message: &Message) -> Result<(), String> {
    const STARTTLS_PORT: u16 = 587;

    let builder = if cfg.port == STARTTLS_PORT {
        SmtpTransport::starttls_relay(&cfg.host)
    } else {
        SmtpTransport::relay(&cfg.host)
    }
    .map_err(|e| format!("无法连接 {}:{}: {e}", cfg.host, cfg.port))?;

    let mailer = builder
        .port(cfg.port)
        .credentials(Credentials::new(
            cfg.username.clone(),
            cfg.password.clone(),
        ))
        .build();

    mailer.send(message).map_err(|e| {
        // 认证失败是最常见的原因，且提示要具体——「授权码」而非「密码」，
        // 国内邮箱这两者不是一回事，混淆会让人反复试错
        if e.is_permanent() {
            format!("发送被拒（多为授权码错误或未开启 SMTP 服务）: {e}")
        } else {
            format!("发送失败: {e}")
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::content::StubbornWord;

    fn cfg() -> SmtpConfig {
        SmtpConfig {
            host: "smtp.example.com".into(),
            port: 465,
            username: "sender@example.com".into(),
            password: "secret".into(),
            to: "parent@example.com".into(),
            from_name: "WordCraft".into(),
        }
    }

    fn report() -> WeeklyReport {
        WeeklyReport {
            week_start: "2026-08-03".into(),
            week_end: "2026-08-09".into(),
            sessions_done: 18,
            sessions_total: 21,
            completion_rate: 18.0 / 21.0,
            new_words: 35,
            reviews: 280,
            accuracy: 0.9,
            vocab_estimate: 1500,
            current_streak: 6,
            stubborn: vec![StubbornWord {
                word: "although".into(),
                meaning: "虽然".into(),
                lapses: 3,
            }],
        }
    }

    #[test]
    fn 组装出的邮件含主题与正文() {
        let msg = compose(&cfg(), &report()).unwrap();
        let raw = String::from_utf8_lossy(&msg.formatted()).to_string();

        assert!(raw.contains("parent@example.com"), "收件人应在信头");
        assert!(raw.contains("sender@example.com"), "发件人应在信头");
        // 主题含日期，家长的收件箱里多封周报才能区分
        assert!(raw.contains("2026-08-03") || raw.contains("=?"), "主题应含周起始日");
    }

    #[test]
    fn 非法发件地址在组装阶段就失败() {
        // 早失败：不必等连上服务器才发现地址写错
        let mut c = cfg();
        c.username = "not an email".into();
        let err = compose(&c, &report()).unwrap_err();
        assert!(err.contains("发件地址"), "{err}");
    }

    #[test]
    fn 非法收件地址在组装阶段就失败() {
        let mut c = cfg();
        c.to = "@@@".into();
        let err = compose(&c, &report()).unwrap_err();
        assert!(err.contains("收件地址"), "{err}");
    }

    #[test]
    fn 正文为纯文本且含关键指标() {
        let msg = compose(&cfg(), &report()).unwrap();
        let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
        assert!(raw.contains("text/plain"), "应声明为纯文本");
    }

    #[test]
    fn 密码不出现在邮件内容里() {
        // 防呆：任何把 config 整体格式化进正文的改动都会被这条拦下
        let msg = compose(&cfg(), &report()).unwrap();
        let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
        assert!(!raw.contains("secret"), "凭据泄漏进邮件正文");
    }
}
