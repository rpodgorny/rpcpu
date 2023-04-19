use anyhow::Result;

const PREFIX: &str = "/sys/devices/system/cpu/cpufreq/policy0";
const AC_FN: &str = "/sys/class/power_supply/AC0/online";
const LVLS: [&str; 5] = ["fix", "min", "mid", "max", "max+"];
const SLEEP: u64 = 2;

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
    let cur = my_read_to_string(fn_).unwrap_or_else(|_| "max".to_string());
    let idx = LVLS.iter().position(|&x| x == cur).unwrap_or(0);
    let nxt = LVLS[(idx + 1) % LVLS.len()];
    ensure_file_content(fn_, nxt)?;
    make_writeable(fn_)?;
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

    let state_dir = "/tmp/cpu_freq_crop";
    let state_fn = format!("{state_dir}/state");
    if !std::path::Path::new(state_dir).is_dir() {
        std::fs::create_dir_all(state_dir)?;
    }
    make_writeable("/tmp/cpu_freq_crop")?;
    if let Some(cmd) = std::env::args().nth(1) {
        let res = match cmd.as_str() {
            "cycle" | "toggle" => cycle(&state_fn),
            _ => Err(anyhow::anyhow!("unknown command")),
        };
        return res;
    }

    ensure_file_content(format!("{PREFIX}/scaling_governor"), "powersave")?;
    // TODO: also: /sys/devices/system/cpu/cpu*/power/energy_perf_bias
    // TODO: read possible values from /sys/devices/system/cpu/cpufreq/policy0/energy_performance_available_preferences
    //ensure_file_content(format!("{PREFIX}/energy_performance_preference"), "balance_power")?;

    let freq_min = my_read_to_string(format!("{PREFIX}/cpuinfo_min_freq"))?;
    let freq_max = my_read_to_string(format!("{PREFIX}/cpuinfo_max_freq"))?;
    let freq_base = my_read_to_string(format!("{PREFIX}/base_frequency"))?;

    let mut ac_status_last = my_read_to_string(AC_FN)?;
    let mut lvl_last = my_read_to_string(&state_fn).unwrap_or_else(|_| "max".to_string());
    loop {
        let ac_status = my_read_to_string(AC_FN)?;
        if ac_status != ac_status_last {
            if ac_status == "1" {
                ensure_file_content(&state_fn, "max+")?;
            } else {
                ensure_file_content(&state_fn, "mid")?;
            }
            make_writeable(&state_fn)?;
            ac_status_last = ac_status;
        }
        let lvl = my_read_to_string(&state_fn).unwrap_or_else(|_| "max".to_string());
        if lvl != lvl_last {
            let (min_, max_, no_turbo, perf_pref) = match lvl.as_str() {
                "fix" => (&freq_base, &freq_base, "0", "balance_power"),
                "min" => (&freq_min, &freq_min, "1", "balance_power"),
                "mid" => (&freq_min, &freq_base, "1", "balance_power"),
                "max" => (&freq_min, &freq_max, "0", "balance_power"),
                "max+" => (&freq_min, &freq_max, "0", "performance"),
                //_ => todo!(),
                _ => unreachable!(),
            };
            ensure_file_content(format!("{PREFIX}/scaling_min_freq"), min_)?;
            ensure_file_content(format!("{PREFIX}/scaling_max_freq"), max_)?;
            ensure_file_content("/sys/devices/system/cpu/intel_pstate/no_turbo", no_turbo)?;
            ensure_file_content(format!("{PREFIX}/energy_performance_preference"), perf_pref)?;
            lvl_last = lvl;
        }
        std::thread::sleep(std::time::Duration::from_secs(SLEEP));
    }
}
