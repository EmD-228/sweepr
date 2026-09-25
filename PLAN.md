# Sweepr — plan d'exécution

Ce plan découpe la v0.1 (SPEC, section 11) en lots livrables, chacun testé avant de passer au suivant. Les versions suivantes sont esquissées à la fin. Le SPEC reste la référence pour le *quoi* ; ce fichier décrit le *comment* et l'ordre.

Légende : `[x]` fait, `[~]` en cours, `[ ]` à faire.

## Principes d'implémentation

- **Le cœur Rust ne dépend pas de Tauri.** Tout le moteur (catalogue, scan, garde-fous, plan, exécution, mesure) vit dans des modules purs, testables avec `cargo test`. La couche Tauri (`commands.rs`) ne fait que traduire. Cela prépare aussi la future interface en ligne de commande.
- **Le catalogue est une donnée.** Fichiers TOML dans `src-tauri/catalog/`, embarqués dans le binaire (`include_str!`) et validés au démarrage *et* par un test : une règle invalide casse le build de tests, pas l'app chez l'utilisateur.
- **Aucune commande construite par concaténation.** Une commande du catalogue est un `argv` fixe (`program` + `args`), exécutée avec `std::process::Command` sans shell. Les seuls paramètres variables (un identifiant de simulateur, un nom de volume Docker) viennent d'un *fournisseur* Rust qui les a lus lui-même depuis l'outil, et sont validés par un motif strict.
- **Chaque suppression passe par `safety::check_deletable`** : chemin absolu, canonique, sous le dossier personnel ou une racine autorisée par la règle, jamais une racine protégée (`/`, `~`, `~/Documents`…), pas de lien symbolique à la cible.
- **Simulation = même code que l'exécution**, avec un exécuteur qui liste au lieu d'agir.

## Architecture du cœur Rust

```
src-tauri/
  catalog/                    règles en TOML (données)
    macos.toml                grand public macOS
    windows.toml              grand public Windows (v0.2)
    dev-tools.toml            caches et outils développeur
    ecosystems.toml           artefacts de projets par écosystème
  src/
    catalog/                  modèle des règles, chargement, validation
    paths.rs                  expansion des jetons (~, %LOCALAPPDATA%…), globs
    safety.rs                 validation des chemins avant suppression
    size.rs                   calcul de taille parallèle, liens physiques, taille allouée
    platform/                 espace libre, dossiers système, par OS
    projects/                 détection de projets, gestionnaire de paquets, activité
    guards/                   git (fichiers suivis, imbriqués, non commités), .env, processus
    providers/                éléments listés par un outil : simulateurs, runtimes, AVD, NDK, Docker
    plan.rs                   sélection → plan d'actions (estimation, risques, pertes)
    exec.rs                   exécution élément par élément, statut de chacun
    measure.rs                gain réel : espace libre avant, suivi jusqu'à stabilisation
    history.rs                historique cumulé (v1.0)
    commands.rs               commandes Tauri
```

### Format d'une règle

```toml
[[rule]]
id = "macos.app-caches"
profile = "general"            # general | developer
platforms = ["macos"]
title = "Caches des applications"
summary = "Données temporaires que les applications recréent toutes seules."
risk = 0                       # 0 à 3, SPEC section 6
loses = "Rien. Certaines applications seront un peu plus lentes au premier lancement."
regenerate = "Automatique."
group_by = "child"             # présenter par sous-dossier (par application)

[rule.target]
kind = "paths"                 # paths | command | provider
paths = ["~/Library/Caches/*"]
```

Types de cibles :

| `kind` | Usage | Exemple |
|---|---|---|
| `paths` | supprimer des fichiers ou dossiers désignés par des globs | Corbeille, caches, DerivedData |
| `command` | lancer une commande officielle au `argv` fixe | `npm cache clean --force`, `docker builder prune -a -f` |
| `provider` | éléments listés par du code (un par simulateur, par volume…) | simulateurs iOS, NDK, projets Docker |

Les écosystèmes de projets (SPEC 5.4) ont leur propre table : fichiers de détection, dossiers d'artefacts, commande officielle, consigne de régénération.

## v0.1 — moteur et macOS

### Lot 0 — mise en place
- [x] `git init`, branche `feat/v0.1-engine`
- [x] `pnpm install`
- [ ] README en anglais
- [ ] premier `cargo check` et `cargo test` verts

### Lot 1 — fondations du moteur
- [ ] modèle des règles, chargement TOML, validation (identifiants uniques, risque 0–3, globs sous une racine autorisée, `argv` sans métacaractères de shell)
- [ ] `paths` : expansion des jetons, globs, chemins avec espaces
- [ ] `safety` : racines protégées, liens symboliques, chemins hors du dossier personnel
- [ ] `size` : parcours parallèle, taille allouée (`st_blocks`), déduplication des liens physiques par (périphérique, inode)
- [ ] `platform` : espace libre du volume (`statvfs` sur Unix, `GetDiskFreeSpaceExW` sur Windows)
- [ ] `measure` : échantillonnage jusqu'à stabilisation, logique testée avec une horloge et un échantillonneur simulés

### Lot 2 — catalogues
- [ ] `macos.toml` : tout le tableau SPEC 5.1
- [ ] `dev-tools.toml` : tout le tableau SPEC 5.5 (hors éléments `provider`)
- [ ] `ecosystems.toml` : tout le tableau SPEC 5.4
- [ ] test qui charge et valide tous les catalogues

### Lot 3 — projets et garde-fous
- [ ] découverte des projets par fichier manifeste, sans descendre dans les artefacts
- [ ] gestionnaire de paquets par fichier de verrouillage, alerte si plusieurs
- [ ] version de Node attendue (`.nvmrc`, `engines`)
- [ ] activité : dernier fichier modifié hors artefacts, dernier commit ; séparation actif / inactif (30 jours)
- [ ] garde-fous git : `git ls-files -- <dossier>`, dépôts imbriqués, `git status --short`, `git log --branches --not --remotes`
- [ ] `.env` jamais supprimés lors d'un nettoyage d'artefacts
- [ ] tests sur de vrais dépôts git temporaires

### Lot 4 — fournisseurs développeur
- [ ] simulateurs iOS (`xcrun simctl list devices -j`), état « Booted » = en cours
- [ ] runtimes iOS (`xcrun simctl runtime list -j`), signaler ceux qu'aucun simulateur n'utilise
- [ ] émulateurs Android (`~/.android/avd`), image système reliée par `image.sysdir.1`, émulateur en cours
- [ ] NDK Android relié aux projets (`ndkVersion`, version par défaut de Flutter, React Native)
- [ ] Docker par projet (label `com.docker.compose.project`), images `<none>` rattachées, images partagées, volumes orphelins, Docker non lancé

### Lot 5 — plan, simulation, exécution, mesure
- [ ] plan : sélection → actions, gain estimé, risque maximal, pertes, régénération
- [ ] exécution élément par élément : fait / ignoré / erreur et pourquoi
- [ ] fichiers verrouillés ou droits insuffisants : ignorés et signalés, jamais bloquants
- [ ] mesure du gain réel branchée sur l'exécution

### Lot 6 — commandes Tauri
- [ ] `scan_overview`, `list_projects`, `simulate`, `execute`, `measure_status`
- [ ] événements de progression (scan en deux temps, exécution par élément)
- [ ] scan annulable

### Lot 7 — interface
- [ ] système de design : jetons, typographie, clair et sombre, icônes `@lucide/svelte`
- [ ] écran de scan (résultat rapide puis détail)
- [ ] vue d'ensemble triée par gain possible, mode simple sans jargon
- [ ] vue Projets : actifs / inactifs, sélection de groupe, exclusions
- [ ] confirmation : gain, risques, pertes, régénération ; cases à cocher pour le risque 2, double confirmation pour le risque 3
- [ ] exécution et résultat avec gain réel, « récupération en cours »
- [ ] demande de l'accès complet au disque sur macOS

### Lot 8 — vérification de bout en bout
- [ ] dossier personnel factice (fixtures) pour tester scan, simulation et exécution sans toucher au vrai disque
- [ ] passe de finition de l'interface
- [ ] build signé local

## Versions suivantes

- **v0.2 — Windows** : `windows.toml`, jetons `%TEMP%`, `%LOCALAPPDATA%`, élévation des droits pour DISM, Windows Update et `powercfg`, compactage du `.vhdx` Docker/WSL.
- **v1.0** : corbeille temporaire de 7 jours pour le risque 3, historique des gains, réactivation des projets, distribution signée et notarisée.
- **Ensuite** : archivage, veille dans la barre des menus, Linux, interface en ligne de commande.

## Journal

- 2026-09-25 : plan rédigé. Lots 0, 1, 2, 3, 5 et 6 faits (43 tests). Lot 7 : première version de l'interface (vue d'ensemble, projets, outils, simulation, confirmation à double étape, exécution, gain réel). Reste : lot 4 (fournisseurs : simulateurs, émulateurs, NDK, Docker, gros fichiers, doublons, sauvegardes iPhone), lot 8.
- Choix v0.1 : en attendant la corbeille de 7 jours (v1.0), les risques 2 et 3 vont dans la corbeille du système.
- Découvert au scan réel : une app lancée depuis le Finder n'a pas le `PATH` du shell (`process::search_path`) ; le SDK Flutter ressemblait à un projet (`projects::is_tool_install`).
