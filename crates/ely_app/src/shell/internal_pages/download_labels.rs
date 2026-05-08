use ely_domain::{
    DownloadDestination, DownloadEntry, DownloadPolicy, DownloadSecurity, DownloadState,
};

pub(crate) fn download_state_label(state: &DownloadState) -> &'static str {
    match state {
        DownloadState::InProgress => "In progress",
        DownloadState::Paused => "Paused",
        DownloadState::Completed => "Complete",
        DownloadState::Cancelled => "Cancelled",
        DownloadState::Failed => "Failed",
    }
}

pub(crate) fn download_size_label(entry: &DownloadEntry) -> String {
    match entry.total_bytes() {
        Some(total_bytes) => {
            format!("{} of {}", format_bytes(entry.received_bytes()), format_bytes(total_bytes))
        }
        None => format_bytes(entry.received_bytes()),
    }
}

pub(crate) fn download_policy_label(policy: &DownloadPolicy) -> String {
    format!("Profile path: {}", download_destination_label(policy.destination()))
}

pub(crate) fn download_entry_location_label(entry: &DownloadEntry) -> String {
    match entry.target_file_path() {
        Some(path) => path.display().to_string(),
        None => download_destination_label(entry.destination()),
    }
}

pub(crate) fn download_security_label(security: &DownloadSecurity) -> &'static str {
    match security {
        DownloadSecurity::Standard => "Standard",
        DownloadSecurity::DangerousExtension => "Extension prompt required",
    }
}

fn download_destination_label(destination: &DownloadDestination) -> String {
    match destination {
        DownloadDestination::AskEveryTime => "Ask before saving".to_string(),
        DownloadDestination::FixedDirectory(path) => path.display().to_string(),
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes >= GIB {
        return format!("{:.1} GB", bytes as f64 / GIB as f64);
    }
    if bytes >= MIB {
        return format!("{:.1} MB", bytes as f64 / MIB as f64);
    }
    if bytes >= KIB {
        return format!("{:.1} KB", bytes as f64 / KIB as f64);
    }
    format!("{bytes} B")
}
