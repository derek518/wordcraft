// Repository 层建成于 T07，但接线要到 T08（排队）/ T09（commit_review）/ T10（统计）
// 才完成——在此之前所有函数从 command 层看都是未使用的。
//
// 这个 allow 的移除条件是明确的：T10 结束时全部 repo 函数都已被 command 调用，
// 届时必须删掉它，让真正的死代码重新可见。
#![allow(dead_code)]

//! Repository 层：每个模块封装一张表（或一组紧密相关的表）的读写。
//!
//! 约定：
//! * 函数接受 `&Connection`，不持有连接——事务边界由调用方决定，
//!   这样 T09 的 `commit_review` 才能把多张表的写入放进同一个事务。
//! * 受控值在此校验并给出可诊断消息；数据库 CHECK 是最后防线，不是唯一防线。
//! * 一切时间运算走 `db::clock`，模块内不出现手写日历逻辑（ADR-4）。

pub mod homestead;
pub mod player_stats;
pub mod review_logs;
pub mod sessions;
pub mod settings;
pub mod word_states;
pub mod words;
