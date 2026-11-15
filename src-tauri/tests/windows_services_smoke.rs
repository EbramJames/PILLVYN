#[cfg(test)]
mod integration_tests {
    use pillvyn_lib::config::PillvynConfig;
    use pillvyn_lib::services::downloads::DownloadMonitor;
    use pillvyn_lib::services::gaming::GamingMonitor;
    use pillvyn_lib::services::hardware::HardwareCollector;
    use pillvyn_lib::services::media::{query_media_snapshot, ThumbnailCache};
    use pillvyn_lib::services::network::NetworkMonitor;
    use pillvyn_lib::services::system_ctrl::query_system_snapshot;
    use pillvyn_lib::services::widgets::WidgetEngine;

    #[test]
    fn hardware_collector_reads_real_system_stats() {
        let mut hw = HardwareCollector::new();
        let snap = hw.collect();
        assert!(snap.ram_total_bytes > 0);
        assert!(snap.cpu_cores > 0);
        assert!(!snap.disks.is_empty());
    }

    #[test]
    fn system_controls_and_network_smoke_test() {
        let mut cached_b = None;
        let sys = query_system_snapshot(true, &mut cached_b);
        assert!(sys.output_volume >= 0.0 && sys.output_volume <= 1.0);

        let mut net = NetworkMonitor::new();
        let net_snap = net.collect();
        assert!(!net_snap.adapter_name.is_empty());
    }

    #[test]
    fn media_gaming_downloads_and_widgets_smoke_test() {
        let mut thumb = ThumbnailCache::default();
        let _media = query_media_snapshot(&mut thumb);

        let cfg = PillvynConfig::default();
        let mut gm = GamingMonitor::new();
        let (_gm_snap, _fg) = gm.collect(&cfg.gaming);

        let mut dl = DownloadMonitor::new();
        let _dl_snap = dl.collect();

        let mut widgets = WidgetEngine::new();
        let timer = widgets.add_timer("Test".into(), 60);
        assert_eq!(timer.remaining_seconds, 60);
        assert!(widgets.toggle_timer(&timer.id));
        assert!(widgets.remove_timer(&timer.id));
    }
}
