//! 旧库导入测试：拿真实的 `~/.podara/podara.db` 跑一遍 dry-run + 真导入。
//!
//! 依赖本机那一份老库，所以默认 #[ignore]：
//!   cargo test --test legacy_import -- --ignored --nocapture

use std::path::PathBuf;

use dear_universe_podcast_lib::{
    core::{db::Database, legacy},
};

/// 40MB 的老库就在本机，来回跑一次大概几百毫秒
fn source_path() -> PathBuf {
    legacy::default_path().expect("本机没有 %USERPROFILE%")
}

/// 临时目录 + 一个全新空库
fn fresh_target(name: &str) -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("建临时目录失败");
    let db = Database::open_in(dir.path()).unwrap_or_else(|e| panic!("开库失败：{e}"));
    assert!(dir.path().join("dear-universe.db").exists(), "{name}: 没落下 db 文件");
    (dir, db)
}

#[test]
#[ignore = "需要本机老库：cargo test --test legacy_import -- --ignored --nocapture"]
fn scan_reports_expectable_numbers() {
    let report = legacy::scan(&source_path()).expect("scan 失败");

    println!("源数据: {:?}", report.source);
    println!("将导入: {:?}", report.plan);
    println!("将跳过: {:?}", report.skipped);

    assert!(report.usable, "老库应该可用：{:?}", report.reason);

    // 老库是**活文件**（Podara 自己还在跑，行数会变），所以除了订阅数这种
    // 稳定的量，其余一律用「守恒关系」断言，别把当时的行数写死 —— 写死了
    // 过两天就红，红的还不是真 bug。
    assert_eq!(report.source.podcasts, 21, "订阅数对不上");
    assert!(report.source.episodes > 4000, "单集数少得离谱：{}", report.source.episodes);

    // 全部单集的播客都在库里，所以不该有孤儿单集被丢
    assert_eq!(report.skipped.orphan_episode, 0);
    assert_eq!(
        report.plan.episodes + report.skipped.orphan_episode,
        report.source.episodes,
        "单集行数不守恒"
    );

    // 进度：只有 positionMs>0 且单集还在的才迁
    assert!(report.plan.play_states >= 4, "能迁的进度太少了");
    assert_eq!(
        report.plan.play_states + report.skipped.empty_play_state
            + report.skipped.orphan_play_state,
        report.source.play_states,
        "进度行数不守恒"
    );

    // 队列：一条都不少（匹配不上单集的退化成 raw_url，不能丢）
    assert_eq!(
        report.plan.queue_items, report.source.queue_items,
        "队列项被丢了"
    );
}

#[test]
#[ignore = "需要本机老库：cargo test --test legacy_import -- --ignored --nocapture"]
fn import_writes_everything_and_can_repeat() {
    let (_guard, db) = fresh_target("import");
    let data_dir = db_dir(&_guard);

    let report = legacy::import(&db, &source_path(), &data_dir).expect("导入失败");
    println!("写入: {:?}", report.plan);
    println!("跳过: {:?}", report.skipped);
    println!("耗时: {}ms 报告: {:?}", report.elapsed_ms, report.report_path);

    let expected = legacy::scan(&source_path()).expect("scan 失败").plan;

    assert_eq!(report.plan.podcasts, 21, "订阅没导全");
    assert_eq!(report.plan.episodes, expected.episodes, "单集没导全");
    assert_eq!(report.plan.play_states, expected.play_states, "进度没导全");
    assert_eq!(report.plan.queue_items, expected.queue_items, "队列没导全");
    assert!(report_report_exists(report.report_path.as_deref()), "没写 import-report.json");

    // 库里的真实行数
    assert_eq!(count(&db, "SELECT COUNT(*) FROM podcast"), 21);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM episode"), expected.episodes as i64);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM play_state"), expected.play_states as i64);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM queue_item"), expected.queue_items as i64);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM subscription"), 21);

    // §8.2：老库的 duration 单位是秒，库里必须 ×1000 存成毫秒。
    // 用样本不够 tricky：老库 progress 表里的 durationMs 本来就是毫秒、更是精确，
    // 会掩盖「忘了乘」的 bug，所以这里对照 episode 表自己那份。
    let ms = duration_ms(&db, SAMPLE_EPISODE);
    println!("样本单集 duration_ms = {ms}");
    assert_eq!(ms, SAMPLE_DURATION_SEC * 1000, "duration 没乘 1000");

    // 队列里那一条匹配不上单集的，必须退化成 raw_url 而不是丢掉（§8.5）
    let raw = count(&db, "SELECT COUNT(*) FROM queue_item WHERE episode_id IS NULL");
    assert_eq!(raw, 1, "应该有一条裸 URL 的队列项");

    // 重复导入：upsert 语义，行数不增长，且不报错
    let again = legacy::import(&db, &source_path(), &data_dir).expect("二次导入失败");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM podcast"), 21, "第二次导入后订阅变多了");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM episode"), expected.episodes as i64);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM play_state"),
        expected.play_states as i64,
        "重复导入不该新增进度"
    );
    assert_eq!(again.plan.play_states, 0, "已有进度应当保留不被覆盖");
}

#[test]
#[ignore = "需要本机老库：cargo test --test legacy_import -- --ignored --nocapture"]
fn scan_refuses_non_podara_file() {
    let dir = tempfile::tempdir().expect("建临时目录失败");
    let fake = dir.path().join("fake.db");
    // 一个能开但不是 Podara 库的 SQLite 文件
    let conn = rusqlite::Connection::open(&fake).expect("开假库失败");
    conn.execute_batch("CREATE TABLE t (a INTEGER)").expect("建表失败");
    drop(conn);

    let report = legacy::scan(&fake).expect("scan 不该报错");
    assert!(!report.usable, "不是 Podara 库却说可用");
    println!("拒绝理由: {:?}", report.reason);
}

fn db_dir(guard: &tempfile::TempDir) -> PathBuf {
    guard.path().to_path_buf()
}

fn report_report_exists(path: Option<&str>) -> bool {
    path.map(std::path::Path::new).is_some_and(|p| p.exists())
}

fn count(db: &Database, sql: &str) -> i64 {
    let conn = db.read().expect("拿连接失败");
    conn.query_row(sql, [], |r| r.get(0)).expect(sql)
}

/// 老库里这条单集的 `duration` 是 5214（秒）
const SAMPLE_EPISODE: &str =
    "https://sspai.typlog.io/episodes/feed.xml:https://sspai.typlog.io/episodes/tim-and-nike";
const SAMPLE_DURATION_SEC: i64 = 5214;

fn duration_ms(db: &Database, id: &str) -> i64 {
    let conn = db.read().expect("拿连接失败");
    conn.query_row("SELECT duration_ms FROM episode WHERE id = ?1", [id], |r| r.get(0))
        .unwrap_or_else(|e| panic!("取不到 {id} 的时长：{e}"))
}
