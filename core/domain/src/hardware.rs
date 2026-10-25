use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Hardware {
    pub cpu_name: String,
    pub recommended: String,
    pub explanation: String,
    pub choices: Vec<String>,
    pub berserk_recommended: String,
    pub berserk_explanation: String,
    pub berserk_choices: Vec<String>,
    pub obsidian_recommended: String,
    pub obsidian_choices: Vec<String>,
    pub reckless_recommended: String,
    pub reckless_choices: Vec<String>,
    pub igel_recommended: String,
    pub igel_choices: Vec<String>,
    pub cpp_recommended: String,
    pub cpp_choices: Vec<String>,
}

pub fn detect() -> Hardware {
    use std::arch::x86_64::__cpuid;
    let mut brand = Vec::new();
    let mut vendor = Vec::new();
    // CPUID is available on every supported x64 Windows CPU. Rust feature
    // detection additionally checks OS support for extended register state.
    let (family, max) = {
        let vendor_leaf = __cpuid(0);
        for word in [vendor_leaf.ebx, vendor_leaf.edx, vendor_leaf.ecx] {
            vendor.extend(word.to_le_bytes());
        }
        let leaf = __cpuid(1).eax;
        let base = (leaf >> 8) & 15;
        (
            base + if base == 15 { (leaf >> 20) & 255 } else { 0 },
            __cpuid(0x80000000).eax,
        )
    };
    if max >= 0x80000004 {
        for leaf in 0x80000002..=0x80000004 {
            let info = __cpuid(leaf);
            for word in [info.eax, info.ebx, info.ecx, info.edx] {
                brand.extend(word.to_le_bytes());
            }
        }
    }
    let slow_bmi2 = vendor == b"AuthenticAMD" && family <= 0x17;
    let mut choices = vec!["native".to_owned(), "x86-64".to_owned()];
    let mut add = |id: &str, supported: bool| {
        if supported {
            choices.push(id.into());
        }
    };
    let pop = std::is_x86_feature_detected!("popcnt");
    let avx2 = pop && std::is_x86_feature_detected!("avx2");
    let bmi2 = avx2 && std::is_x86_feature_detected!("bmi2");
    add(
        "x86-64-sse3-popcnt",
        pop && std::is_x86_feature_detected!("sse3"),
    );
    add("x86-64-ssse3", std::is_x86_feature_detected!("ssse3"));
    add(
        "x86-64-sse41-popcnt",
        pop && std::is_x86_feature_detected!("sse4.1"),
    );
    add("x86-64-avx2", avx2);
    add("x86-64-bmi2", bmi2);
    let avx512 = bmi2
        && std::is_x86_feature_detected!("avx512f")
        && std::is_x86_feature_detected!("avx512bw");
    add(
        "x86-64-avxvnni",
        bmi2 && std::is_x86_feature_detected!("avxvnni"),
    );
    add("x86-64-avx512", avx512);
    add(
        "x86-64-vnni512",
        avx512
            && std::is_x86_feature_detected!("avx512vnni")
            && std::is_x86_feature_detected!("avx512dq")
            && std::is_x86_feature_detected!("avx512vl"),
    );
    let recommended = recommend(&choices, slow_bmi2);
    let berserk_choices = berserk_targets(&choices);
    let berserk_recommended = berserk_recommend(&berserk_choices, slow_bmi2);
    let mut obsidian_choices = vec!["native".into()];
    if pop {
        obsidian_choices.push("sse2".into());
    }
    if pop && std::is_x86_feature_detected!("ssse3") {
        obsidian_choices.push("ssse3".into());
    }
    let obs_avx2 = avx2
        && std::is_x86_feature_detected!("ssse3")
        && std::is_x86_feature_detected!("sse4.1")
        && std::is_x86_feature_detected!("bmi1")
        && std::is_x86_feature_detected!("fma");
    if obs_avx2 {
        obsidian_choices.push("avx2".into());
    }
    if obs_avx2 && bmi2 {
        obsidian_choices.push("avx2-pext".into());
    }
    if obs_avx2 && avx512 {
        obsidian_choices.push("avx512".into());
    }
    let obsidian_recommended = ["avx512", "avx2-pext", "avx2", "ssse3", "sse2", "native"]
        .into_iter()
        .find(|id| {
            !(slow_bmi2 && matches!(*id, "avx512" | "avx2-pext"))
                && obsidian_choices.iter().any(|c| c == id)
        })
        .unwrap()
        .into();
    let mut reckless_choices = vec!["native".into(), "x86-64".into()];
    if std::is_x86_feature_detected!("avx2") {
        reckless_choices.push("avx2".into());
    }
    let reckless_recommended = if reckless_choices.iter().any(|c| c == "avx2") {
        "avx2"
    } else {
        "x86-64"
    }
    .into();
    let mut igel_choices = Vec::new();
    if avx2 {
        igel_choices.extend(["native".into(), "avx2".into()]);
    }
    if bmi2 {
        igel_choices.push("avx2-pext".into());
    }
    let igel_recommended = if bmi2 && !slow_bmi2 {
        "avx2-pext"
    } else if avx2 {
        "avx2"
    } else {
        ""
    }
    .into();
    let mut cpp_choices = vec!["x86-64".into()];
    if obs_avx2 && std::is_x86_feature_detected!("sse4.2") {
        cpp_choices.push("avx2".into());
        if bmi2 {
            cpp_choices.push("bmi2".into());
        }
    }
    let cpp_recommended = if cpp_choices.iter().any(|v| v == "bmi2") && !slow_bmi2 {
        "bmi2"
    } else if cpp_choices.iter().any(|v| v == "avx2") {
        "avx2"
    } else {
        "x86-64"
    }
    .into();
    Hardware {
        cpu_name: String::from_utf8_lossy(&brand)
            .trim_matches(['\0', ' '])
            .to_owned(),
        explanation: format!(
            "Recommended Stockfish target: {recommended}. Only targets supported by this CPU and Windows are offered. {}This is a feature-based recommendation; benchmark builds to compare actual speed.",
            if slow_bmi2 {
                "BMI2 is available but not preferred on older AMD processors. "
            } else {
                ""
            }
        ),
        berserk_explanation: format!(
            "Recommended Berserk target: {berserk_recommended}. Berserk names its targets differently from Stockfish; -pext adds BMI2 move generation. {}native uses Berserk's own detection.",
            if slow_bmi2 {
                "PEXT is available but slow on older AMD processors, so it is not recommended. "
            } else {
                ""
            }
        ),
        berserk_recommended,
        berserk_choices,
        obsidian_recommended,
        obsidian_choices,
        reckless_recommended,
        reckless_choices,
        igel_recommended,
        igel_choices,
        cpp_recommended,
        cpp_choices,
        recommended,
        choices,
    }
}

/// Berserk's makefile ARCH values for the targets this CPU supports. Its
/// x86-64 target is baseline 64-bit plus POPCNT.
fn berserk_targets(choices: &[String]) -> Vec<String> {
    let has = |id: &str| choices.iter().any(|c| c == id);
    let mut targets = vec!["native".to_owned(), "x86-64".to_owned()];
    for (stockfish, berserk) in [
        ("x86-64-sse41-popcnt", "sse41"),
        ("x86-64-avx2", "avx2"),
        ("x86-64-bmi2", "avx2-pext"),
        ("x86-64-avx512", "avx512"),
        ("x86-64-avx512", "avx512-pext"),
    ] {
        if has(stockfish) {
            targets.push(berserk.into());
        }
    }
    targets
}

fn berserk_recommend(targets: &[String], slow_bmi2: bool) -> String {
    for id in ["avx512-pext", "avx512", "avx2-pext", "avx2", "sse41"] {
        if id.ends_with("-pext") && slow_bmi2 {
            continue;
        }
        if targets.iter().any(|t| t == id) {
            return id.into();
        }
    }
    "x86-64".into()
}

fn recommend(choices: &[String], slow_bmi2: bool) -> String {
    for id in [
        "x86-64-vnni512",
        "x86-64-avx512",
        "x86-64-avxvnni",
        "x86-64-bmi2",
        "x86-64-avx2",
        "x86-64-sse41-popcnt",
        "x86-64-ssse3",
        "x86-64-sse3-popcnt",
        "x86-64",
    ] {
        if id == "x86-64-bmi2" && slow_bmi2 {
            continue;
        }
        if choices.iter().any(|c| c == id) {
            return id.into();
        }
    }
    "x86-64".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zen_avoids_slow_pext_and_recommendation_is_supported() {
        let choices = ["x86-64", "x86-64-avx2", "x86-64-bmi2"].map(str::to_owned);
        assert_eq!(recommend(&choices, true), "x86-64-avx2");
        assert_eq!(recommend(&choices, false), "x86-64-bmi2");
        let hardware = detect();
        assert!(hardware.choices.contains(&hardware.recommended));
        assert!(
            hardware
                .berserk_choices
                .contains(&hardware.berserk_recommended)
        );
    }
    #[test]
    fn berserk_targets_map_to_its_makefile_names() {
        let choices = ["native", "x86-64", "x86-64-avx2", "x86-64-bmi2"].map(str::to_owned);
        let targets = berserk_targets(&choices);
        assert_eq!(targets, ["native", "x86-64", "avx2", "avx2-pext"]);
        assert_eq!(berserk_recommend(&targets, true), "avx2");
        assert_eq!(berserk_recommend(&targets, false), "avx2-pext");
        assert_eq!(berserk_recommend(&targets[..2], false), "x86-64");
    }
}
