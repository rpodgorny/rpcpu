use anyhow::Result;

const PREFIX: &str = "/sys/devices/system/cpu/cpufreq/policy0";
const LVLS: [&str; 4] = ["fix", "min", "mid", "max"];

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
    let content = my_read_to_string(&p)?;
    if content != v {
        //let p_ = p.as_ref().to_str().unwrap();
        //log::debug!("{p_}: {content} -> {v}");
        let p_ = p.as_ref().as_os_str();
        log::debug!("{p_:?}: {content} -> {v}");
        std::fs::write(&p, v)?;
    }
    Ok(())
}

fn cycle(fn_: &str) {
    let cur = my_read_to_string(fn_).unwrap_or_else(|_| "max".to_string());
    let idx = LVLS.iter().position(|&x| x == cur).unwrap_or(0);
    let nxt = LVLS[(idx + 1) % LVLS.len()];
    ensure_file_content(fn_, nxt).unwrap();
}

fn main() {
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

    let state_fn = "/tmp/cpu_freq_crop";
    if std::env::args().len() > 1 && std::env::args().nth(1).unwrap() == "toggle" {
        cycle(state_fn);
        return;
    }

    let freq_min = my_read_to_string(format!("{PREFIX}/cpuinfo_min_freq")).unwrap();
    let freq_max = my_read_to_string(format!("{PREFIX}/cpuinfo_max_freq")).unwrap();
    let freq_base = my_read_to_string(format!("{PREFIX}/base_frequency")).unwrap();
    loop {
        ensure_file_content(format!("{PREFIX}/scaling_governor"), "powersave").unwrap();
        ensure_file_content(format!("{PREFIX}/energy_performance_preference"), "balance_power").unwrap(); // TODO: read possible values from /sys/devices/system/cpu/cpufreq/policy0/energy_performance_available_preferences
        // TODO: also: /sys/devices/system/cpu/cpu*/power/energy_perf_bias
        let lvl = my_read_to_string(state_fn).unwrap_or_else(|_| "max".to_string());
        // TODO: unfinished shit
        let (min_, max_, no_turbo) = match lvl.as_str() {
            "fix" => (&freq_base, &freq_base, "0"),
            "min" => (&freq_min, &freq_min, "1"),
            "mid" => (&freq_min, &freq_base, "1"),
            "max" => (&freq_min, &freq_max, "0"),
            _ => todo!(),
        };
        ensure_file_content(format!("{PREFIX}/scaling_min_freq"), min_).unwrap();
        ensure_file_content(format!("{PREFIX}/scaling_max_freq"), max_).unwrap();
        ensure_file_content("/sys/devices/system/cpu/intel_pstate/no_turbo", no_turbo).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
