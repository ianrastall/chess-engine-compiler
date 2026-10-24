use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Hardware {
    pub cpu_name: String,
    pub recommended: String,
    pub explanation: String,
    pub choices: Vec<String>,
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
        recommended,
        choices,
    }
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
    }
}
