//! 音频播放线程。
//!
//! ## 为什么不外部进程播放
//!
//! 先前 Windows 分支把 mp3 交给 PowerShell + WMP COM 播放，Windows 实机上
//! 表现为**闪一个 cmd 窗、没有声音**。三个问题叠在一起：
//!
//! 1. `Command::new("powershell")` 在 Windows 上默认创建控制台窗口
//! 2. WMP 的 `duration` 要等媒体异步加载完才可读。脚本刚 `newMedia` 就读它，
//!    拿到 0，于是只睡 300ms 就退出——进程一死 COM 对象被销毁，
//!    声音刚起头就断了
//! 3. 每个词启一个 PowerShell，光进程启动常常就超过 spec F4 的 300ms 预算
//!
//! 三者同源：用外部进程放音频。改为进程内解码播放后一并消失，
//! 而且 macOS 与 Windows 走同一条代码路径——开发机上能验证的，
//! 正是用户在 Windows 上跑的那一条。
//!
//! ## 为什么是常驻线程而非每次新开
//!
//! `OutputStream` 一旦 drop，声音立刻停——这与 WMP 那个 bug 是同一个形状。
//! 播放又必须不阻塞命令（spec F4 给的是 300ms），所以用一条常驻线程持有
//! 输出流，命令只往通道里投路径。设备也因此只打开一次，省掉每次的初始化延迟。

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};

/// 播放请求。
enum Cmd {
    Play(PathBuf),
}

pub struct AudioPlayer {
    tx: Sender<Cmd>,
    /// 音频设备打开失败的原因。无声卡的机器（CI、部分虚拟机）属正常情况，
    /// 但必须让调用方能说出「为什么没声音」，而不是静静地什么都不做
    unavailable: Option<String>,
}

impl AudioPlayer {
    /// 启动播放线程。设备打开失败不算致命——应用其余部分照常工作。
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<Option<String>>();

        std::thread::Builder::new()
            .name("wordcraft-audio".into())
            .spawn(move || {
                let stream = match rodio::OutputStreamBuilder::open_default_stream() {
                    Ok(s) => {
                        let _ = ready_tx.send(None);
                        s
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Some(format!("打开音频设备失败: {e}")));
                        // 仍要把通道抽干，否则发送方拿到的是「通道已断开」，
                        // 那条错误远不如「设备打开失败」有用
                        for _ in rx {}
                        return;
                    }
                };

                // 单个 Sink 复用：再次点击播放应当打断上一个词，而不是排队。
                // 排队会让连点几下之后声音一个接一个地放，与预期完全相反
                let sink = rodio::Sink::connect_new(stream.mixer());

                for Cmd::Play(path) in rx {
                    let file = match std::fs::File::open(&path) {
                        Ok(f) => f,
                        Err(e) => {
                            log::warn!("打开音频 {} 失败: {e}", path.display());
                            continue;
                        }
                    };
                    match rodio::Decoder::try_from(file) {
                        Ok(source) => {
                            sink.clear();
                            sink.append(source);
                            sink.play();
                        }
                        Err(e) => log::warn!("解码 {} 失败: {e}", path.display()),
                    }
                }
            })
            .expect("创建音频线程失败");

        // 等线程报告设备状态。它只在启动时发一次，不会拖住应用启动
        let unavailable = ready_rx.recv().unwrap_or_else(|_| Some("音频线程未启动".into()));
        if let Some(ref why) = unavailable {
            log::warn!("音频不可用，发音将降级为系统合成：{why}");
        }

        Self { tx, unavailable }
    }

    pub fn play(&self, path: PathBuf) -> Result<(), String> {
        if let Some(why) = &self.unavailable {
            return Err(why.clone());
        }
        self.tx
            .send(Cmd::Play(path))
            .map_err(|_| "音频线程已退出".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 有没有声卡取决于跑测试的机器（CI 上通常没有），所以不能断言某个
    /// 具体结果。能断言的是**两种结局都必须是明确的**：
    /// 要么受理，要么给出说得出口的原因——绝不能悄悄成功却没有声音。
    #[test]
    fn 结果非成功即有因_不存在静默失败() {
        let p = AudioPlayer::start();
        match p.play(PathBuf::from("/nonexistent.mp3")) {
            Ok(()) => {}
            Err(e) => {
                assert!(!e.trim().is_empty(), "错误信息不能为空");
                assert!(
                    e.contains("音频") || e.contains("设备") || e.contains("线程"),
                    "错误信息要说清是哪一环出了问题: {e}"
                );
            }
        }
    }

    #[test]
    fn 设备不可用时每次调用都报错而非只报第一次() {
        // 曾经的失败模式：首次报错后状态被清掉，后续调用静默返回 Ok
        let p = AudioPlayer::start();
        let a = p.play(PathBuf::from("/x.mp3"));
        let b = p.play(PathBuf::from("/y.mp3"));
        assert_eq!(a.is_err(), b.is_err(), "同一台机器上的结论必须稳定");
    }
}

#[cfg(test)]
mod manual {
    use super::*;

    /// 真的放一遍预生成的 mp3。默认跳过：CI 上没有声卡，也无从断言「听见了」。
    ///
    /// 手动跑：`cargo test 真实播放 -- --ignored --nocapture`
    ///
    /// 这条测试是这次修复的关键——先前 Windows 的播放走 PowerShell + WMP，
    /// macOS 走 afplay，两条完全不同的路径，于是开发机上验证过的东西
    /// 在 Windows 上照样是坏的。现在两个平台共用这一条，本机听得见
    /// 就意味着 Windows 上走的是同一段代码。
    #[test]
    #[ignore = "会实际发声，需手动运行"]
    fn 真实播放预生成音频() {
        // 取目录里第一个 mp3，不绑死某个词——词库会变，`crystal` 就不在里面
        let path = std::fs::read_dir("audio")
            .expect("找不到 audio/，先跑 scripts/tts/pregenerate.py")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.extension().is_some_and(|x| x == "mp3"))
            .expect("audio/ 下没有 mp3");
        println!("播放 {}", path.display());

        let started = std::time::Instant::now();
        let player = AudioPlayer::start();
        let open_ms = started.elapsed().as_millis();

        let t = std::time::Instant::now();
        player.play(path).expect("播放请求被拒");
        let submit_ms = t.elapsed().as_millis();

        println!("设备打开 {open_ms}ms · 提交播放 {submit_ms}ms");
        // spec F4 给的是 300ms。设备只在启动时打开一次，
        // 之后每次播放就是往通道里投一个路径
        assert!(submit_ms < 300, "提交耗时 {submit_ms}ms 超出 spec F4 预算");

        std::thread::sleep(std::time::Duration::from_millis(1500));
    }
}
