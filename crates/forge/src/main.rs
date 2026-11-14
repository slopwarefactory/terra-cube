//! forge: makes creatures. One command takes a creature from a line of text to an animated
//! model the game loads, and says in a few lines whether it is good. docs/pipeline.md.
//!
//! Map: `spec` (the files) · `tripo` (paid steps, once each) · `finish` (stand, name and make
//! clips, measure) · `rig` (skeleton, roles) · `clips` (clip recipes) · `check` (the bars) ·
//! `shaped` (a rig made here for what flies) · `glb` (the file format) · `cubes` (a model rebuilt from little cubes).

mod built;
mod check;
mod clips;
mod crawl;
mod cubes;
mod finish;
mod glb;
mod grip;
mod library;
mod looks;
mod own;
mod strider;
mod rig;
mod shaped;
mod spec;
mod tripo;
mod walker;

use anyhow::{Context, Result, bail};
use spec::{Creature, Lock, Meta, Style};
use std::path::Path;
use std::process::{Command, ExitCode};

const HELP: &str = "\
cargo forge status                      every creature: steps done, credits, checks
cargo forge new ID --kind K --plan P --height M \"SUBJECT\"
                                        write assets/creatures/ID/creature.toml
cargo forge prompt ID                   the concept prompt a build would send
cargo forge concepts [ID…]               shots/concepts.jpg: every concept image in one picture,
                                        in the order listed (all, by id, when none is named)
cargo forge build ID… [--to STEP]       do what is missing: concept, model, rig, export (Tripo,
                                        paid once), then finish and check. --to concept stops
                                        after the free-ish image, to look before paying
cargo forge finish ID…|--all            the local half again (after changing clips or style)
cargo forge check ID…|--all             the checks alone
cargo forge sheet ID [--view side]      render shots/ID.png, every clip at six moments
cargo forge cubes ID [--size 0.05] [--head 1.0]
                                        a trial: ID-cubes, that model rebuilt from little cubes each
                                        fixed to one of a dozen parts, with the same skeleton and
                                        clips; --head 1.4 makes its head that much bigger
cargo forge ideas ID                    shots/ideas-ID.jpg: other things that creature could be, each
                                        subject in its ideas.toml drawn in the house style (2 credits
                                        an image, once); --take NAME makes that one what it is
cargo forge looks                       shots/looks.jpg: a few creatures drawn in each look being tried
                                        (assets/creatures/looks.toml); 2 credits an image, once
cargo forge inspect FILE.glb|ID         the skeleton's roles
cargo forge doctor                      is this machine set up: node, Tripo login, display, models
kinds: player monster wildlife · plans: see assets/creatures/style.toml";

fn style() -> Result<Style> {
    spec::load(&Path::new(spec::ROOT).join("style.toml"))
}

fn targets(args: &[String]) -> Vec<String> {
    if args.iter().any(|a| a == "--all") { spec::ids() } else { args.iter().filter(|a| !a.starts_with("--")).cloned().collect() }
}

fn option(args: &mut Vec<String>, name: &str) -> Option<String> {
    let at = args.iter().position(|a| a == name)?;
    args.remove(at);
    (at < args.len()).then(|| args.remove(at))
}

/// Finishes and checks one creature; prints its verdict lines. True when every check passed.
fn finish_and_check(id: &str, style: &Style, write: bool) -> Result<bool> {
    // A skeleton forge cannot make sense of must not stop the others being finished: a crash
    // in the clip maker is that creature's failure, said in a line.
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let done = std::panic::catch_unwind(|| finish_and_check_one(id, style, write));
    std::panic::set_hook(quiet);
    done.unwrap_or_else(|_| anyhow::bail!("forge crashed making its clips: the skeleton is one it cannot read (`cargo forge inspect {id}`)"))
}

fn finish_and_check_one(id: &str, style: &Style, write: bool) -> Result<bool> {
    let creature: Creature = spec::load(&spec::dir(id).join("creature.toml"))?;
    let plan = style.plans.get(&creature.plan).with_context(|| format!("no plan `{}`", creature.plan))?;
    let done = if write { finish::run(id, style)? } else { finish::finish(&spec::work(id).join("rigged.glb"), &creature, plan, style.checks.max_slide)? };
    let wanted: Vec<String> = plan.clips.iter().filter(|c| !creature.without.contains(c)).cloned().collect();
    let lines = check::check(&done, &wanted, &style.checks, plan.max_stretch.unwrap_or(style.checks.max_stretch), creature.slide.unwrap_or(style.checks.max_slide))?;
    let failed = lines.iter().filter(|l| !l.ok).count();
    println!("{id}: {} · {:.2} m tall, {:.2} m long · walk {:.2} m/s, run {:.2} m/s", if failed == 0 { "ok".into() } else { format!("{failed} FAILED") }, done.meta.height, done.meta.length, done.meta.walk_speed, done.meta.run_speed);
    for l in &lines {
        // Passing lines are noise once a creature is good; show them only with a failure.
        if !l.ok || failed > 0 || std::env::var("FORGE_VERBOSE").is_ok() {
            println!("  {}", l.text);
        }
    }
    Ok(failed == 0)
}

fn status(style: &Style) -> Result<()> {
    println!("{:<14} {:<9} {:<10} {:<28} {:>7}  model", "id", "kind", "plan", "tripo steps", "credits");
    let mut total = 0.0;
    for id in spec::ids() {
        let c: Creature = spec::load(&spec::dir(&id).join("creature.toml"))?;
        let lock_path = spec::dir(&id).join("forge.lock");
        let lock: Lock = if lock_path.exists() { spec::load(&lock_path)? } else { Lock::default() };
        let credits: f64 = lock.credits.values().sum::<f64>().abs();
        total += credits;
        let meta_path = spec::dir(&id).join("meta.toml");
        let model = if meta_path.exists() {
            let m: Meta = spec::load(&meta_path)?;
            format!("{} tris, {} joints, {} clips", m.triangles, m.joints, m.clips.len())
        } else {
            "not finished".into()
        };
        let plan_ok = if style.plans.contains_key(&c.plan) { "" } else { " (unknown plan)" };
        println!("{:<14} {:<9} {:<10} {:<28} {:>7}  {model}", id, c.kind, format!("{}{plan_ok}", c.plan), lock.steps.join(","), credits);
    }
    println!("{total} credits in all. Next: `cargo forge build ID` for a creature whose steps are short; `cargo forge check --all`.");
    Ok(())
}

fn run() -> Result<bool> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "help" {
        println!("{HELP}");
        return Ok(true);
    }
    let command = args.remove(0);
    let mut ok = true;
    match command.as_str() {
        "status" => status(&style()?)?,
        "doctor" => ok = doctor(),
        "looks" => println!("{}", looks::run(&style()?)?),
        "ideas" => {
            let style = style()?;
            if let Some(name) = option(&mut args, "--take") {
                let id = targets(&args).into_iter().next().context("ideas --take wants a creature: cargo forge ideas player --take plain")?;
                println!("{}", looks::take(&id, &name)?);
                return Ok(ok);
            }
            for id in targets(&args) {
                println!("{}", looks::ideas(&id, &style)?);
            }
        }
        "cubes" => {
            let size: f32 = option(&mut args, "--size").map_or(Ok(0.05), |s| s.parse())?;
            let head: f32 = option(&mut args, "--head").map_or(Ok(1.0), |s| s.parse())?;
            let style = style()?;
            for id in targets(&args) {
                let (summary, lines) = cubes::run(&id, size, head, &style)?;
                println!("{summary}");
                let failed = lines.iter().any(|l| !l.0);
                for (passed, text) in &lines {
                    if !passed || (failed && std::env::var_os("FORGE_VERBOSE").is_some()) || std::env::var_os("FORGE_VERBOSE").is_some() {
                        println!("  {text}");
                    }
                }
                ok &= !failed;
            }
        }
        "new" => {
            let kind = option(&mut args, "--kind").context("--kind player|monster|wildlife")?;
            let plan = option(&mut args, "--plan").context("--plan")?;
            let height: f32 = option(&mut args, "--height").context("--height METRES")?.parse()?;
            let [id, subject] = &args[..] else { bail!("give an id and a subject\n{HELP}") };
            let dir = spec::dir(id);
            if dir.join("creature.toml").exists() {
                bail!("{id} exists");
            }
            std::fs::create_dir_all(&dir)?;
            let mut name = id.replace(['-', '_'], " ");
            name[..1].make_ascii_uppercase();
            let c = Creature { name, kind, plan, subject: subject.clone(), height, span: None, built: None, facing: "auto".into(), speeds: None, walked: Vec::new(), rests: String::new(), low: 0.0, gait_gain: 1.0, slide: None, ease: 1.0, made: Vec::new(), hops: false, without: Vec::new(), look: None, look_of: None };
            spec::save(&dir.join("creature.toml"), &c, "")?;
            println!("wrote {}. Next: cargo forge build {id} --to concept", dir.join("creature.toml").display());
        }
        "prompt" => {
            let style = style()?;
            for id in targets(&args) {
                let c: Creature = spec::load(&spec::dir(&id).join("creature.toml"))?;
                println!("{}", tripo::prompt(&c, style.plans.get(&c.plan).context("unknown plan")?, &style));
            }
        }
        "concepts" => {
            let ids = if args.is_empty() { spec::ids() } else { targets(&args) };
            let ids: Vec<String> = ids.into_iter().filter(|id| spec::dir(id).join("concept.jpg").exists()).collect();
            let cols = ids.len().clamp(1, 4) as u32;
            let rows = (ids.len() as u32).div_ceil(cols).max(1);
            let mut sheet = image::RgbImage::new(cols * 320, rows * 320);
            for (i, id) in ids.iter().enumerate() {
                let thumb = image::open(spec::dir(id).join("concept.jpg"))?.thumbnail(320, 320).to_rgb8();
                image::imageops::replace(&mut sheet, &thumb, (i as u32 % cols * 320) as i64, (i as u32 / cols * 320) as i64);
            }
            std::fs::create_dir_all("shots")?;
            sheet.save("shots/concepts.jpg")?;
            println!("shots/concepts.jpg left to right, top to bottom: {}", ids.join(" "));
        }
        "build" => {
            let until = option(&mut args, "--to").unwrap_or_else(|| "export".into());
            if !tripo::STEPS.contains(&until.as_str()) {
                bail!("--to takes one of {}", tripo::STEPS.join(", "));
            }
            let style = style()?;
            for id in targets(&args) {
                let c: Creature = spec::load(&spec::dir(&id).join("creature.toml"))?;
                let plan = style.plans.get(&c.plan).with_context(|| format!("{id}: no plan `{}`", c.plan))?;
                let one = (|| -> Result<bool> {
                    let spent = tripo::run(&id, &c, plan, &style, &until)?;
                    if until != "export" {
                        println!("{id}: stopped after {until} ({spent} credits). Look at {}", spec::dir(&id).join("concept.jpg").display());
                        return Ok(true);
                    }
                    let good = finish_and_check(&id, &style, true)?;
                    println!("  {spent} credits this run. Next: cargo forge sheet {id}");
                    Ok(good)
                })();
                // One creature's failure does not stop the others.
                ok &= one.unwrap_or_else(|e| {
                    println!("{id}: FAILED {e:#}");
                    false
                });
            }
        }
        "finish" | "check" => {
            let style = style()?;
            for id in targets(&args) {
                ok &= finish_and_check(&id, &style, command == "finish").unwrap_or_else(|e| {
                    println!("{id}: FAILED {e:#}");
                    false
                });
            }
        }
        "sheet" => {
            let view = option(&mut args, "--view");
            let clips = option(&mut args, "--clips");
            for id in targets(&args) {
                let mut cmd = Command::new("cargo");
                cmd.args(["run", "--quiet", "--release", "--package", "cube_game", "--", "--shot", &id]);
                if let Some(v) = &view {
                    cmd.args(["--view", v]);
                }
                if let Some(c) = &clips {
                    cmd.args(["--clips", c]);
                }
                let out = cmd.output()?;
                let text = String::from_utf8_lossy(&out.stdout);
                match text.lines().find(|l| l.starts_with("shot ")) {
                    Some(l) => println!("{l}"),
                    None => {
                        ok = false;
                        println!("{id}: FAILED no sheet; {}", String::from_utf8_lossy(&out.stderr).lines().rev().take(3).collect::<Vec<_>>().join(" | "));
                    }
                }
            }
        }
        "inspect" => {
            let target = args.first().context("give a .glb or a creature id")?;
            let path = if target.ends_with(".glb") { Path::new(target).to_path_buf() } else { spec::work(target).join("rigged.glb") };
            let glb = glb::Glb::read(&path)?;
            let sk = rig::Skeleton::read(&glb)?;
            let facing = option(&mut args, "--facing").unwrap_or_else(|| "auto".into());
            print!("{}", rig::Roles::read(&sk, rig::Facing::parse(&facing)?).describe(&sk));
            for c in clips::read(&glb, &sk)? {
                println!("clip   {} {:.2}s", c.name, c.seconds());
            }
        }
        other => bail!("unknown command {other}\n{HELP}"),
    }
    Ok(ok)
}

/// Is this machine set up to build creatures and run the sandbox. Says what to do about
/// each thing that is not.
fn doctor() -> bool {
    let mut lines = Vec::new();
    let mut ok = true;
    let mut report = |good: bool, name: &str, detail: String| {
        ok &= good;
        lines.push(format!("{} {name} {detail}", if good { "ok" } else { "FAIL" }));
    };

    let node = Command::new("node").arg("--version").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap_or_default();
    let mut parts = node.trim_start_matches('v').split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let version = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    let detail = if node.is_empty() { "not found; install Node 22.18 or newer".into() } else if version < (22, 18) { format!("{node}; needs 22.18 or newer") } else { node.clone() };
    report(version >= (22, 18), "node", detail);

    match tripo::balance() {
        Ok(credits) => report(true, "tripo", format!("{credits} credits")),
        Err(e) => report(false, "tripo", format!("{}; see tools/tripo/README.md", format!("{e:#}").lines().next().unwrap_or(""))),
    }

    let display = std::env::var("WAYLAND_DISPLAY").or_else(|_| std::env::var("DISPLAY")).unwrap_or_default();
    let needed = cfg!(target_os = "linux");
    let detail = if !display.is_empty() { display.clone() } else if needed { "none; on a server run `Xvfb :1 -screen 0 1920x1080x24` and set DISPLAY=:1".into() } else { "native".into() };
    report(!needed || !display.is_empty(), "display", detail);

    let ids = spec::ids();
    let built = ids.iter().filter(|id| spec::dir(id).join("model.glb").exists()).count();
    report(built > 0, "creatures", format!("{built} of {} have a model", ids.len()));

    // In WSL the game runs best as a Windows program, which needs a Windows linker here.
    let wsl = std::fs::read_to_string("/proc/version").is_ok_and(|v| v.to_lowercase().contains("microsoft"));
    if wsl {
        let linker = Command::new("x86_64-w64-mingw32-gcc").arg("--version").output().is_ok_and(|o| o.status.success());
        report(linker, "windows", if linker { "linker found".into() } else { "no linker for `make windows`; sudo apt install mingw-w64".into() });
    }

    for line in &lines {
        if !ok || line.starts_with("FAIL") || std::env::var_os("FORGE_VERBOSE").is_some() {
            println!("{line}");
        }
    }
    if ok {
        println!("ok: {}. Start with: make play", lines.iter().map(|l| l.trim_start_matches("ok ")).collect::<Vec<_>>().join(" · "));
    }
    ok
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("forge: {e:#}");
            ExitCode::from(2)
        }
    }
}
