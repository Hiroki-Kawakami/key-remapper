use std::process::Command;

const KEYBOARD_PAGE: u64 = 0x7_0000_0000;

pub fn set_mapping(location_id: u32, mapping: &[(u16, u16)]) -> Result<(), String> {
    let entries: Vec<String> = mapping
        .iter()
        .map(|(src, dst)| {
            format!(
                "{{\"HIDKeyboardModifierMappingSrc\":{},\"HIDKeyboardModifierMappingDst\":{}}}",
                KEYBOARD_PAGE | u64::from(*src),
                KEYBOARD_PAGE | u64::from(*dst)
            )
        })
        .collect();
    let matching =
        format!("{{\"LocationID\":{location_id},\"PrimaryUsagePage\":1,\"PrimaryUsage\":6}}");
    let property = format!("{{\"UserKeyMapping\":[{}]}}", entries.join(","));
    let output = Command::new("/usr/bin/hidutil")
        .args(["property", "--matching", &matching, "--set", &property])
        .output()
        .map_err(|e| format!("hidutil: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "hidutil {matching}: {} {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}
