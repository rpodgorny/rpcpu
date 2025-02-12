use anyhow::Result;

const PREFIX: &str = "/sys/devices/system/cpu/cpufreq/policy";
const AC_FN: &str = "/sys/class/power_supply/AC0/online";
const LVLS: [&str; 5] = ["fix", "min", "mid", "max", "max+"];
const LVL_DEFAULT: &str = "mid";
const SLEEP: u64 = 2;
const DEBOUNCE: u64 = 6;

fn my_read_to_string<P>(p: P) -> Result<String>
where
    P: AsRef<std::path::Path>,
{
    Ok(std::fs::read_to_string(p)?.trim().to_string())
}

fn ensure_file_content<P>(p: P, v: &str) -> Result<()>
where
    P: AsRef<std::path::Path>,
{
    let content = my_read_to_string(&p).unwrap_or_default();
    if content != v {
        //let p_ = p.as_ref().to_str().unwrap();
        //log::debug!("{p_}: {content} -> {v}");
        let p_ = p.as_ref().as_os_str();
        log::debug!("{p_:?}: {content} -> {v}");
        std::fs::write(&p, v)?;
    }
    Ok(())
}

fn make_writeable(fn_: &str) -> Result<()> {
    // TODO: setting the perms does not really help since /tmp is mounted with sticky flag
    let mut perms = std::fs::metadata(fn_)?.permissions();
    perms.set_readonly(false);
    std::fs::set_permissions(fn_, perms)?;
    Ok(())
}

fn cycle(fn_: &str) -> Result<()> {
    let nxt = my_read_to_string(fn_).map_or(LVL_DEFAULT, |cur| {
        let idx = LVLS.iter().position(|&x| x == cur).unwrap_or(0);
        LVLS[(idx + 1) % LVLS.len()]
    });
    ensure_file_content(fn_, nxt)?;
    std::process::Command::new("/usr/bin/notify-send")
        .args(["-t", "1000", nxt])
        .status()?;
    Ok(())
}

fn main() -> Result<()> {
    simplelog::TermLogger::init(
        //simplelog::LevelFilter::Trace,
        //simplelog::LevelFilter::Info,
        simplelog::LevelFilter::Debug,
        simplelog::Config::default(),
        simplelog::TerminalMode::default(),
        simplelog::ColorChoice::Auto,
    )
    .unwrap();

    log::info!("starting rpautovpn v{}", env!("CARGO_PKG_VERSION"));

    let state_fn = "/tmp/cpu_freq_crop/state".to_string();
    let state_dir = std::path::Path::new(&state_fn).parent().unwrap();
    if !state_dir.is_dir() {
        log::info!("will create directory {}", state_dir.display());
        std::fs::create_dir_all(state_dir)?;
        make_writeable(state_dir.to_str().unwrap())?;
    }
    if !std::path::Path::new(&state_fn).exists() {
        log::info!("will create file {}", state_fn);
        ensure_file_content(&state_fn, LVL_DEFAULT)?;
        make_writeable(&state_fn)?;
    }
    if let Some(cmd) = std::env::args().nth(1) {
        let res = match cmd.as_str() {
            "cycle" | "toggle" => cycle(&state_fn),
            _ => Err(anyhow::anyhow!("unknown command")),
        };
        return res;
    }

    let prefixes: Vec<_> = (0..99)
        .map(|x| format!("{PREFIX}{x}"))
        .filter(|x| std::path::Path::new(x).exists())
        .collect();

    for prefix in &prefixes {
        ensure_file_content(format!("{prefix}/scaling_governor"), "powersave")?;
    }
    // TODO: also: /sys/devices/system/cpu/cpu*/power/energy_perf_bias
    // TODO: read possible values from /sys/devices/system/cpu/cpufreq/policy0/energy_performance_available_preferences
    //ensure_file_content(format!("{PREFIX}/energy_performance_preference"), "balance_power")?;

    let freq_min = my_read_to_string(format!("{PREFIX}0/cpuinfo_min_freq"))?;
    let freq_max = my_read_to_string(format!("{PREFIX}0/cpuinfo_max_freq"))?;
    let freq_base = my_read_to_string(format!("{PREFIX}0/base_frequency"))?;

    let mut ac_status_last = my_read_to_string(AC_FN)?;
    let mut ac_change_t = None;
    let mut lvl_last = None;
    loop {
        let ac_status = my_read_to_string(AC_FN)?;
        if ac_status != ac_status_last {
            if ac_change_t.is_none() {
                log::info!("detected ac state change");
                ac_change_t = Some(std::time::Instant::now());
            } else if ac_change_t.unwrap().elapsed() > std::time::Duration::from_secs(DEBOUNCE) {
                log::info!("ac state change debounced");
                if ac_status == "1" {
                    ensure_file_content(&state_fn, "max")?;
                } else {
                    ensure_file_content(&state_fn, "mid")?;
                }
                make_writeable(&state_fn)?;
                ac_status_last = ac_status;
                ac_change_t = None;
            }
        } else if ac_change_t.is_some() {
            log::info!("ac state back to previous value, probably just a fluke");
            ac_change_t = None;
        }
        let lvl = my_read_to_string(&state_fn).ok();
        if lvl != lvl_last && lvl.is_some() {
            log::info!("level change from {:?} to {:?}", lvl_last, lvl);
            // TODO: get rid of the clone and unwrap if possible
            let (min_, max_, no_turbo, perf_pref) = match lvl.clone().unwrap().as_str() {
                "fix" => (&freq_base, &freq_base, "0", "balance_power"),
                "min" => (&freq_min, &freq_min, "1", "balance_power"),
                "mid" => (&freq_min, &freq_base, "1", "balance_power"),
                "max" => (&freq_min, &freq_max, "0", "balance_power"),
                "max+" => (&freq_min, &freq_max, "0", "performance"),
                //_ => todo!(),
                _ => unreachable!(),
            };
            for prefix in &prefixes {
                ensure_file_content(format!("{prefix}/scaling_min_freq"), min_)?;
                ensure_file_content(format!("{prefix}/scaling_max_freq"), max_)?;
                ensure_file_content(format!("{prefix}/energy_performance_preference"), perf_pref)?;
            }
            ensure_file_content("/sys/devices/system/cpu/intel_pstate/no_turbo", no_turbo)?;
        }
        lvl_last = lvl;
        std::thread::sleep(std::time::Duration::from_secs(SLEEP));
    }
}
