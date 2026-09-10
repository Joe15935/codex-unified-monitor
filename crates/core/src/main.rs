use codexmeter_core::{
    account, accounting::Tokens, analytics, ingest, pricing::Catalog, settings::Settings,
    storage::Store,
};
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|x| x == "--help") {
        println!("codexmeter-data [--codex-home PATH] [--data-dir PATH] [--no-ingest] [--snapshot] [--period today|5h|week|month|30d|90d|year|all] [--quota] [--session ID] [--audit --format json|html|csv]\nReads local rollout metadata. --quota explicitly calls read-only official account methods. Output may contain private metadata; do not commit it.");
        return Ok(());
    }
    let arg = |key: &str| {
        args.iter()
            .position(|x| x == key)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let data = arg("--data-dir")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(codexmeter_core::data_dir);
    let home = arg("--codex-home")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(codexmeter_core::codex_home);
    let mut store = Store::open(&data.join("monitor.sqlite3"))?;
    let result = if args.iter().any(|x| x == "--no-ingest") {
        None
    } else {
        Some(ingest::ingest(&mut store, &ingest::discover(&home))?)
    };
    let settings = Settings::load(&store)?;
    let c = store
        .conn
        .query_row("SELECT payload FROM pricing_catalog WHERE id=1", [], |r| {
            r.get::<_, String>(0)
        })
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(Catalog::bundled);
    if args.iter().any(|x| x == "--quota") {
        let mut client = account::Client::start()?;
        let q = client.read(true)?;
        account::save(&store, &q)?;
    }
    if args.iter().any(|x| x == "--audit") {
        let q = account::cached(&store)?;
        let view =
            codexmeter_core::auditor::report(&store, &c, &settings, &q, codexmeter_core::now())?;
        let format = arg("--format").unwrap_or("json".into());
        println!(
            "{}",
            codexmeter_core::auditor::render(&view.evidence, &format)?
        );
        return Ok(());
    }
    if let Some(id) = arg("--session") {
        println!(
            "{}",
            serde_json::to_string_pretty(&analytics::session_detail(&store, &id, &c, &settings)?)?
        );
        return Ok(());
    }
    if args.iter().any(|x| x == "--snapshot") {
        let range = analytics::Range {
            period: arg("--period").unwrap_or("today".into()),
            start: arg("--start"),
            end: arg("--end"),
        };
        let report = analytics::report(&store, &c, &settings, range, codexmeter_core::now())?;
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"report":report,"quota":account::cached(&store)?,"settings":settings,"ingest":result})
            )?
        );
        return Ok(());
    }
    let mut total = Tokens::default();
    let mut models = std::collections::BTreeMap::<String, Tokens>::new();
    let mut event_count = 0;
    store.visit_events(0, i64::MAX, |e| {
        total.add(&e.tokens);
        models.entry(e.model).or_default().add(&e.tokens);
        event_count += 1;
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"ingest":result,"tokens":total,"models":models,"event_count":event_count})
        )?
    );
    Ok(())
}
