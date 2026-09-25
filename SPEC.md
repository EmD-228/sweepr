# Sweepr — cahier des charges et récapitulatif

Ce document rassemble tout ce qui a été décidé avant le début du développement. Il sert de point de départ à quiconque reprend le projet, humain ou agent. Lire ce fichier en entier avant d'écrire du code.

## 1. Origine du projet

L'idée est née d'une vraie session de nettoyage sur le Mac d'un développeur, presque plein : environ 170 Go récupérés sans perdre une seule ligne de code.

Le panneau Stockage de macOS affichait de grosses catégories (« Documents », « Developer ») sans dire ce qu'il y avait dedans. Il se met aussi à jour avec plusieurs minutes de retard. Presque rien de ce qui a été supprimé n'y était identifiable.

Ce qui a réellement libéré de la place pendant la session :

| Élément | Gain réel |
|---|---|
| Dossiers de build et de dépendances de 16 projets (`node_modules`, `.next`, `build`, `target`, `Pods`, `.dart_tool`…) | ~35 Go |
| Simulateurs iOS (25 sur 27 supprimés, puis un de plus) | ~31 Go |
| Projets Docker supprimés (conteneurs, images, volumes) | ~15 Go |
| Cache de build Docker | ~17 Go |
| Émulateurs Android | ~15 Go |
| Xcode DerivedData, `.dartServer`, caches Yarn et CocoaPods | ~20 Go |
| NDK Android inutilisé et images système Android | ~7 Go |

Tout a été fait à la main, en ligne de commande, en vérifiant chaque élément avant de le supprimer. **Sweepr doit faire exactement ça, en cliquant sur des boutons, et pour n'importe qui.**

## 2. Vision

Sweepr est une application de bureau pour **macOS, Windows et Linux** qui montre précisément ce qui occupe le disque et propose des actions de nettoyage. Chaque action est expliquée : ce qu'elle supprime, ce qu'on perd, le niveau de risque et comment revenir en arrière.

**Elle ne s'adresse pas qu'aux développeurs.** La personne sous Windows qui n'a plus d'espace cherche aujourd'hui des tutos qui lui font taper des commandes qu'elle ne comprend pas (« supprimez les fichiers temporaires avec… »). Sweepr remplace ces tutos.

**Argument principal : la confiance.** Face à CCleaner (réputation abîmée par les publicités et les faux avertissements alarmistes) et CleanMyMac, Sweepr est honnête. Pas de peur pour vendre, pas de chiffres gonflés. On explique chaque action et on montre le gain réel.

**Décision : pas d'IA.** L'idée d'une IA locale a été étudiée puis abandonnée. Tout repose sur un moteur de règles déterministe et testé. Ne pas réintroduire d'IA sans décision explicite du mainteneur.

## 3. Principes non négociables

1. **Rien n'est supprimé sans confirmation explicite.** Avant chaque action, on affiche ce qui sera supprimé, l'avantage (espace gagné), l'inconvénient (ce qu'on perd) et le niveau de risque.
2. **Simulation d'abord.** Chaque action peut être lancée « à blanc », pour lister exactement ce qu'elle toucherait.
3. **Le risque vient des règles, jamais d'une estimation floue.** Échelle fixe (section 6).
4. **Les commandes exécutées viennent d'un catalogue vérifié.** L'app ne compose jamais de commande de suppression arbitraire à partir d'une saisie ou d'un chemin non validé.
5. **Toujours mesurer le gain réel** (section 8), jamais seulement l'estimation.
6. **Ce qui ne se régénère pas passe par une corbeille temporaire** (7 jours) avant suppression définitive.
7. **Un langage sans jargon en mode simple.** Pas « `%TEMP%` : 4,2 Go », mais « Fichiers temporaires laissés par vos applications : 4,2 Go. Sans risque, Windows les recrée si besoin. »

## 4. Deux profils dans la même app

- **Mode simple (par défaut)** : fichiers temporaires, caches, corbeille, téléchargements, doublons, gros fichiers, sauvegardes de téléphone, etc. Un utilisateur lambda ne doit jamais voir « DerivedData » ou « NDK ».
- **Module développeur** : s'active automatiquement s'il détecte Xcode, Docker, Node, Flutter, Rust, Android Studio… On peut aussi l'activer à la main. Il ajoute les projets, les SDK, les simulateurs, Docker et les caches de développement.

## 5. Catalogue des actions

Chaque entrée du catalogue décrit : ce qu'on détecte, le chemin ou la commande, le niveau de risque, ce qu'on perd et comment régénérer. Les règles doivent être des **données** (fichiers TOML ou JSON par système), pas du code dispersé, pour pouvoir en ajouter facilement.

### 5.1 Grand public — macOS

| Élément | Emplacement | Risque | Remarque |
|---|---|---|---|
| Corbeille | `~/.Trash` | 0 | |
| Caches des applications | `~/Library/Caches/*` | 0 | présenter par application (Spotify 4 Go, navigateurs 2 à 3 Go…) |
| Journaux | `~/Library/Logs` | 0 | |
| Vieux installateurs | `.dmg`, `.pkg` dans Téléchargements ; `/Applications/Install macOS*.app` | 0 | |
| Gros fichiers et vidéos | Téléchargements, Films, Bureau | 2 | montrer, jamais supprimer automatiquement |
| Doublons | dossiers de l'utilisateur | 2 | comparer par taille puis par empreinte |
| Sauvegardes d'iPhone | `~/Library/Application Support/MobileSync/Backup` | 3 | souvent énormes, personne ne sait où elles sont |
| Pièces jointes de Messages | `~/Library/Messages/Attachments` | 3 | photos et vidéos irremplaçables |
| Pièces jointes de Mail | `~/Library/Containers/com.apple.mail/Data/Library/Mail Downloads` | 1 | |

### 5.2 Grand public — Windows

| Élément | Emplacement ou commande | Risque | Remarque |
|---|---|---|---|
| Fichiers temporaires | `%TEMP%`, `C:\Windows\Temp` | 0 | ignorer les fichiers verrouillés |
| Cache de Windows Update | `C:\Windows\SoftwareDistribution\Download` | 0 | arrêter le service `wuauserv` avant |
| Optimisation de la distribution | cache Delivery Optimization | 0 | |
| Ancienne installation de Windows | `C:\Windows.old` | 1 | empêche le retour à la version précédente |
| Nettoyage des composants | `DISM /Online /Cleanup-Image /StartComponentCleanup` | 1 | droits administrateur |
| Fichier d'hibernation | `hiberfil.sys` via `powercfg -h off` | 1 | désactive la mise en veille prolongée et le démarrage rapide |
| Corbeille | par lecteur | 0 | |
| Cache des miniatures | `%LOCALAPPDATA%\Microsoft\Windows\Explorer\thumbcache_*` | 0 | |
| Rapports de plantage | `%LOCALAPPDATA%\CrashDumps`, `MEMORY.DMP` | 0 | |
| Caches des navigateurs | Chrome, Edge, Firefox, Brave | 0 | |
| Vieux installateurs | `.exe`, `.msi` dans Téléchargements | 0 | |
| Gros fichiers, doublons | dossiers de l'utilisateur | 2 | |

### 5.3 Grand public — Linux (plus tard)

Caches XDG (`~/.cache`), journaux systemd (`journalctl --vacuum-size`), caches de paquets (`apt clean`, `dnf clean all`), anciennes versions de snap et flatpak inutilisées, corbeille (`~/.local/share/Trash`).

### 5.4 Développeurs — artefacts de projets

Détection du type de projet par son fichier manifeste, puis règles propres à chaque écosystème. Toujours préférer la commande officielle quand elle existe.

| Écosystème | Détection | À supprimer | Commande officielle | Régénération |
|---|---|---|---|---|
| Node / Next / Nuxt / Vite | `package.json` | `node_modules`, `.next`, `out`, `.nuxt`, `.output`, `.data`, `dist`, `.turbo`, `.vercel`, `.cache` | — | `install` avec le bon gestionnaire, puis `dev` ou `build` |
| Flutter | `pubspec.yaml` | `build`, `.dart_tool`, `ios/Pods`, `android/.gradle` | `flutter clean` | `flutter pub get`, `pod install`, `flutter run` |
| React Native / Expo | `package.json` avec `react-native` | `android/app/build`, `android/app/.cxx`, `android/.gradle`, `ios/Pods`, `.expo` | — | `install`, `pod install`, `run` |
| Rust / Tauri | `Cargo.toml` | `target`, `src-tauri/target`, `src-tauri/gen/android/app/build` | `cargo clean` | `cargo build` (premier build long) |
| Python | `pyproject.toml`, `requirements.txt` | `.venv`, `venv`, `__pycache__` | — | recréer l'environnement virtuel |

Le **bon gestionnaire de paquets** se déduit du fichier de verrouillage : `package-lock.json` pour npm, `pnpm-lock.yaml` pour pnpm, `yarn.lock` pour Yarn. Signaler quand un projet en a plusieurs (cas réel rencontré).

Afficher aussi la version de Node attendue (`.nvmrc`, champ `engines`) dans la consigne de régénération.

### 5.5 Développeurs — outils et caches globaux

| Élément | Emplacement ou commande | Risque | Remarque |
|---|---|---|---|
| Cache npm | `npm cache clean --force` | 0 | |
| Store pnpm | `pnpm store prune` | 0 | ne retire que les paquets inutilisés |
| Cache Yarn | `yarn cache clean` | 0 | lancer depuis le dossier personnel : dans un projet configuré pour pnpm, Corepack refuse |
| Cache CocoaPods | `pod cache clean --all` | 0 | |
| Cache Gradle | `~/.gradle/caches`, `~/.gradle/wrapper` | 1 | **ne jamais toucher** `~/.gradle/gradle.properties` (identifiants de signature) |
| Cache Dart/Flutter | `flutter pub cache clean` | 1 | supprime aussi les outils installés avec `dart pub global activate` |
| Analyseur Dart | `~/.dartServer` | 0 | |
| Xcode DerivedData | `~/Library/Developer/Xcode/DerivedData` | 0 | |
| Simulateurs iOS | `xcrun simctl list devices` / `delete` | 1 | liste par appareil, l'utilisateur choisit ceux à garder |
| Images iOS des simulateurs | `xcrun simctl runtime list` / `delete` | 1 | signaler celles qu'aucun simulateur n'utilise |
| Émulateurs Android | `~/.android/avd/*.avd` et `*.ini` | 1 | vérifier qu'aucun émulateur ne tourne |
| Images système Android | `~/Library/Android/sdk/system-images` | 1 | relier chaque image aux émulateurs qui l'utilisent (`image.sysdir.1` dans `config.ini`) |
| NDK Android | `~/Library/Android/sdk/ndk/<version>` | 1 | relier chaque version aux projets qui l'utilisent (voir ci-dessous) |
| Docker : cache de build | `docker builder prune -a -f` | 0 | Docker Desktop doit être lancé |
| Docker : par projet | conteneurs, images, volumes, réseaux | 3 pour les volumes | voir section 5.6 |

**Relier les SDK aux projets.** C'est ce qui a rendu les décisions évidentes pendant la session :

- `ndkVersion = flutter.ndkVersion` renvoie à la version par défaut de Flutter, lisible dans `packages/flutter_tools/gradle/src/main/kotlin/FlutterExtension.kt` de l'installation Flutter (28.2.13676358 pour Flutter 3.38.5).
- React Native 0.81 / Expo 54 utilise le NDK 27.1.12297006.
- Affichage attendu : « NDK 27.0 : utilisé par aucun projet. NDK 28.2 : utilisé par 5 projets Flutter. »

### 5.6 Docker, projet par projet

- Regrouper conteneurs, images, volumes et réseaux avec le label `com.docker.compose.project`.
- Les images sans nom (`<none>`) peuvent appartenir à un projet : les rattacher par leurs labels (cas réel : une image de 11,7 Go était une ancienne version du backend d'un projet).
- **Les volumes contiennent des données** (bases locales, fichiers envoyés). Risque 3 : les montrer à part, avec leur contenu si possible, et une double confirmation.
- Ne pas supprimer une image partagée avec un autre projet (cas réel : `redis:7-alpine` utilisé par deux projets).
- Signaler les volumes orphelins (anonymes, sans conteneur) sans les supprimer d'office.
- Windows : le disque virtuel de Docker/WSL (`.vhdx`) ne rétrécit pas tout seul, il faut le compacter après un nettoyage.

## 6. Échelle de risque

| Niveau | Nom | Signification | Traitement |
|---|---|---|---|
| 0 | Aucun | Cache pur, se recrée tout seul | confirmation simple |
| 1 | Faible | Se recrée, mais coûte du temps, du réseau ou une petite action (retélécharger, recompiler) | confirmation avec la consigne de régénération |
| 2 | À vérifier | Peut contenir du travail : projet sans git, modifications non commitées, gros fichiers personnels | l'utilisateur coche chaque élément |
| 3 | Irremplaçable | Données qui ne se recréent pas : volumes Docker, pièces jointes, sauvegardes, projet entier | corbeille temporaire de 7 jours et double confirmation |

## 7. Garde-fous avant toute suppression dans un projet

Vérifications faites à la main pendant la session, que l'app doit automatiser :

1. **Aucun fichier suivi par git dans le dossier à supprimer.** Utiliser `git ls-files -- <dossier>` depuis le dossier parent. Cas réels : deux dossiers `build` contenaient respectivement 171 et 183 fichiers versionnés. On les a exclus.
2. **Dépôts git imbriqués.** Un projet peut avoir son `.git` dans un sous-dossier (cas réel : `Client Work/client-app/.git`, avec un dossier parent sans git). Chercher git en profondeur, pas seulement à la racine.
3. **Modifications non commitées et commits non poussés** (`git status --short`, `git log --branches --not --remotes`). Obligatoire avant de supprimer un projet entier.
4. **Fichiers `.env`** : ne jamais les supprimer lors d'un nettoyage d'artefacts. Si on supprime un projet entier, prévenir qu'ils seront perdus (cas réel : un projet supprimé avait un `.env` et des modifications non commitées).
5. **Processus en cours** : ne pas toucher à un émulateur, un simulateur ou un conteneur qui tourne.
6. **Chemins avec espaces** (cas réels : `Client Work`, `untitled folder`) : toujours manipuler les chemins de façon sûre.

## 8. Mesurer le gain réel

Deux pièges rencontrés pendant la session :

- **`du` peut surestimer.** Il compte chaque fichier, même quand des blocs sont partagés (clones APFS, liens physiques du store pnpm).
- **macOS rend l'espace en différé.** Après la suppression de 25 simulateurs, l'espace libre n'avait augmenté que de 0,6 Go. Quelques minutes plus tard, le gain était de ~31 Go. Il ne faut donc conclure ni trop tôt, ni sur la seule base de `du`.

Comportement attendu :

1. Lire l'espace libre du volume avant l'action (API du système, équivalent de `df`).
2. Lancer l'action.
3. Continuer à mesurer l'espace libre pendant quelques minutes, jusqu'à stabilisation, en affichant « récupération en cours ».
4. Afficher le gain réel à côté de l'estimation.
5. Tenir un historique cumulé : « Depuis l'installation, Sweepr vous a fait récupérer X Go. »

## 9. Parcours utilisateur

1. **Scan** : un premier résultat rapide (grandes catégories), puis le détail qui se complète en arrière-plan.
2. **Vue d'ensemble** : ce qui occupe le disque, en langage clair, trié par gain possible.
3. **Projets** (module développeur) : triés par activité, c'est-à-dire la date du dernier fichier modifié hors artefacts, plus le dernier commit. Séparer « actifs ce dernier mois » et « inactifs ». Ce tri a été la décision la plus utile de la session.
4. **Sélection** : l'utilisateur coche des éléments ou un groupe (« tous les projets inactifs »), et peut exclure des éléments (cas réel : garder deux projets en cours, nettoyer tous les autres).
5. **Confirmation** : récapitulatif avec gain estimé, risques, ce qu'on perd et la régénération.
6. **Exécution** : progression élément par élément, avec le statut de chacun (fait, ignoré, erreur et pourquoi). Cas réel : Docker n'était pas lancé, Yarn refusait de tourner dans un dossier configuré pour pnpm.
7. **Résultat** : gain réel mesuré (section 8).

Actions complémentaires prévues :

- **Réactiver** un projet : relancer `install`, `flutter pub get`, `pod install` selon son type. Supprimer fait moins peur quand revenir en arrière tient en un clic.
- **Archiver** un projet mort : nettoyer, compresser, puis déplacer vers un disque externe ou le cloud, au lieu de tout supprimer.

## 10. Architecture technique

Le squelette est déjà généré (voir section 13).

- **Tauri 2**, avec un cœur en **Rust** et une interface **SvelteKit + TypeScript** (adaptateur statique).
- **Moteur de scan en Rust** : parcours parallèle du disque, calcul des tailles, qui prend en compte les liens physiques pour ne pas compter deux fois.
- **Moteur de règles** : les catalogues de la section 5 sont des fichiers de données (TOML ou JSON) par système et par écosystème, chargés au démarrage. Chaque règle précise sa détection, sa cible, sa commande éventuelle, son niveau de risque, ce qu'on perd et comment régénérer.
- **Exécuteur d'actions** : il n'exécute que des actions du catalogue, avec des chemins validés. Aucune commande construite par concaténation de texte.
- **Couche par plateforme** : chemins et commandes propres à macOS, Windows et Linux isolés derrière une interface commune.
- **À terme, une interface en ligne de commande** qui partage le même moteur.

Distribution :

- macOS : l'app a besoin de l'**accès complet au disque**, ce qui complique le passage par le Mac App Store. Prévoir une distribution directe, signée et notarisée.
- Windows : certaines actions demandent les droits administrateur (DISM, Windows Update, `powercfg`).

## 11. Découpage en versions

1. **v0.1 — moteur et macOS** : scan, moteur de règles, garde-fous, catalogue développeur complet et catalogue grand public macOS, simulation, confirmation, exécution, gain réel mesuré.
2. **v0.2 — Windows** : catalogue grand public Windows et adaptation du catalogue développeur (chemins `%LOCALAPPDATA%`, disque virtuel Docker/WSL).
3. **v1.0 — première version publique** : corbeille temporaire, historique des gains, réactivation des projets, distribution signée.
4. **Ensuite** : archivage, veille en arrière-plan avec icône dans la barre des menus et notifications (« 3 projets inactifs depuis 30 jours, 12 Go récupérables »), Linux, interface en ligne de commande.

Pistes de modèle économique : cœur gratuit, voire open source, avec des fonctions payantes (veille en arrière-plan, archivage cloud). Non tranché.

## 12. Concurrence

| Outil | Ce qu'il fait | Ce qui lui manque |
|---|---|---|
| Panneau Stockage de macOS | grandes catégories | pas de détail par dossier, lent, aucune action ciblée |
| Assistant de stockage de Windows | fichiers temporaires | ne couvre pas les outils de développement, peu explicatif |
| CCleaner | nettoyage grand public | réputation abîmée par les publicités et les avertissements alarmistes |
| CleanMyMac | nettoyage grand public sur Mac | payant, opaque sur les développeurs |
| DaisyDisk, WizTree | visualiser l'occupation du disque | ne savent pas ce qui est régénérable |
| npkill | `node_modules` uniquement | un seul écosystème |
| DevCleaner for Xcode | Xcode uniquement | un seul outil |
| kondo | artefacts de projets | ni caches globaux, ni SDK, ni Docker, ni grand public |

Aucun ne réunit projets, SDK, simulateurs, Docker, caches et nettoyage grand public, avec le risque et la régénération de chaque élément. C'est le positionnement de Sweepr.

## 13. État du projet

- Projet open source (licence MIT), publié sous le nom EmD Sama. Identifiant de l'application : `com.emdsama.sweepr`.
- Avancement détaillé : voir [PLAN.md](PLAN.md). Règles de contribution : voir [CONTRIBUTING.md](CONTRIBUTING.md).

## 14. Conventions du projet

- **Git** : jamais de commit direct sur `main`. Toujours une branche, puis une pull request vers `main`.
- **README et documentation publique** en anglais. Ce cahier des charges et le plan sont en français.
- **Design** : jamais de « eyebrow » (petite étiquette au-dessus d'un titre). Jamais d'emoji comme icône : utiliser une bibliothèque d'icônes (Lucide, via `@lucide/svelte`).
- **Supprimer avec prudence** : regarder la cible avant de supprimer, et demander confirmation pour tout ce qui est irréversible. C'est aussi la philosophie du produit.
