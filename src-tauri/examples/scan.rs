//! Read-only scan of the real disk, printed in the terminal. Deletes nothing.
//!
//! cargo run --release --example scan

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use sweepr_lib::catalog::{Catalog, Os};
use sweepr_lib::names::Namer;
use sweepr_lib::paths::PathEnv;
use sweepr_lib::scan;
use sweepr_lib::size::Seen;

fn human(bytes: u64) -> String {
    let gb = bytes as f64 / 1e9;
    if gb >= 1.0 { format!("{gb:.1} Go") } else { format!("{:.0} Mo", bytes as f64 / 1e6) }
}

fn main() {
    let catalog = Catalog::embedded().expect("valid catalog");
    let env = PathEnv::from_system().expect("home folder");
    let cancel = AtomicBool::new(false);
    let seen = Seen::default();
    let developer = scan::developer_detected(&env);
    println!("Module développeur : {developer}");
    println!("Accès complet au disque : {:?}", sweepr_lib::platform::full_disk_access(env.home()));

    let start = Instant::now();
    let rules = scan::active_rules(&catalog, Os::current(), developer);
    let mut items = scan::rule_items(&env, &rules, &Namer::load(env.home()), &seen, &cancel);
    items.sort_by_key(|i| std::cmp::Reverse(i.size));
    println!("\n== Règles ({:.1} s)", start.elapsed().as_secs_f64());
    for item in items.iter().take(40) {
        println!("{:>9}  r{}  {:<28} {}", human(item.size), item.risk as u8, item.rule, item.title);
    }

    let start = Instant::now();
    let projects = scan::scan_projects(&env, &catalog.ecosystems, &seen, &cancel);
    let mut artifacts = scan::project_items(&projects, &catalog.ecosystems);
    println!("\n== Projets : {} ({:.1} s)", projects.len(), start.elapsed().as_secs_f64());
    let start = Instant::now();
    scan::apply_project_guards(&mut artifacts);
    println!("   garde-fous git : {:.1} s", start.elapsed().as_secs_f64());
    for project in projects.iter().take(25) {
        let pms: Vec<_> = project.package_managers.iter().map(|p| p.name()).collect();
        println!(
            "{:>9}  {:<30} {:?} pm={:?} node={:?}",
            human(project.reclaimable()),
            project.name,
            project.ecosystems,
            pms,
            project.node_version
        );
    }
    println!("\n== Artefacts bloqués par les garde-fous");
    for item in artifacts.iter().filter(|i| i.blocked.is_some()) {
        println!("  {} / {} : {}", item.detail.as_deref().unwrap_or(""), item.title, item.blocked.as_deref().unwrap_or(""));
    }
    let total: u64 = items.iter().map(|i| i.size).sum::<u64>() + artifacts.iter().filter(|i| i.blocked.is_none()).map(|i| i.size).sum::<u64>();
    println!("\nTotal récupérable estimé : {}", human(total));
}
