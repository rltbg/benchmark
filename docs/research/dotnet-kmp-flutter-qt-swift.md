# Fiche de recherche : options UI natives (Avalonia/.NET, KMP/Compose, Flutter, Swift, Qt, autres)

Date de consultation : 2026-10-25. Rédigé par un analyste ingénierie ; tout ce qui n'est pas mesuré ou lu dans un dépôt est marqué par son niveau de preuve.

Légende des preuves : [DEPOT] = lu/compté dans le dépôt cloné ou registre (npm/NuGet/Maven/storage.googleapis.com) ; [OFFICIEL-SNIPPET] = extrait d'un résultat de WebSearch citant une page officielle (la page elle-même était bloquée ou non récupérée) ; [PRESSE] = presse/blog tiers ; [COMMUNAUTAIRE] ; [MEMOIRE] = connaissance générale du modèle, non revérifiée aujourd'hui.

AVERTISSEMENT PERFORMANCE : aucun benchmark n'a été exécuté (consigne : pas de build lourd). Tous les chiffres de CPU/RAM/taille/démarrage ci-dessous sont NON VÉRIFIÉS sauf mention contraire. Aucune mesure propre n'existe dans cette fiche.

## 1. Tableau récapitulatif

| Option | Rendu / runtime | Dernière stable (tag, date) | Licence | Contributeurs 12 m (noms distincts / emails distincts, hors bots notables) | Plateformes (niveau de preuve) |
|---|---|---|---|---|---|
| Avalonia (.NET/F#) | Skia (via SkiaSharp) propre rendu ; CoreCLR/NativeAOT ; GC .NET | 12.1.4, 2026-10-09 (tag, NuGet) ; 12.0.0 le 2026-04-06 | MIT [DEPOT] (licence.md, "Copyright AvaloniaUI OÜ") | 179 / 184 ; 27 avec >=5 commits | Win/macOS/Linux, iOS, Android, wasm : annoncés par le projet [OFFICIEL-SNIPPET] ; maturité par cible non mesurée |
| Compose Multiplatform | Skia via Skiko ; JVM (desktop/Android) ; Kotlin/Native (iOS) ; Kotlin/Wasm (web) | 1.12.2, 2026-10-15 (tag + Maven) ; 1.13.0-beta01 le 2026-10-23 | Apache-2.0 [DEPOT] | 33 / 33 (dépôt CMP seulement ; le gros du code vit dans compose-multiplatform-core, non compté) | Android stable ; iOS stable depuis 1.8.0 (mai 2025) [PRESSE/OFFICIEL] ; desktop JVM stable ; Web : Beta depuis 1.9.0 [OFFICIEL-SNIPPET], aucune annonce de stable trouvée |
| Compose natif sans JVM (communautaire) | fork Compose + Skiko natif | alpha seulement (1.12.10-alpha04, 2026-08-13) | à vérifier (non lu) | 1 auteur (72 commits, tous sur 12 m) | Linux x64/arm64, Windows x64, macOS x64/arm64 (README) ; expérimental |
| Flutter | Impeller (Skia en repli selon plateforme) ; Dart AOT/JIT ; Skwasm/CanvasKit web | 3.47.8, 2026-10-23 (storage.googleapis.com, Dart 3.13.7) | BSD-3-Clause [DEPOT] | 411 / 417 (dont engine-flutter-autoroll, 1972 commits, bot) ; 104 avec >=5 commits | iOS/Android/macOS/Linux/Windows/web : officiel [MEMOIRE + presse] |
| Swift (SwiftUI, SDK Android) | natif Apple ; SDK Android officiel depuis 6.3 ; UI cross-platform tierce | 6.4.1, 2026-10-07 (tag) ; 6.4.0 le 2026-09-13 ; 6.3 le 2026-03-20 | Apache-2.0 avec exception runtime [MEMOIRE] | swift : 274 / 320 (hors swift_jenkins, swift-ci) | SwiftUI = Apple seulement ; Android/WASM/Static Linux = SDK officiels ; UI Linux/Windows = communautaire |
| Skip (Swift -> Android) | SwiftUI iOS + Compose Android | 1.9.19, 2026-10-23 | à vérifier (non lu) ; Skip Fuse gratuit pour indies [PRESSE] | 8 / 8 (1 auteur principal, ~100 commits) | iOS + Android |
| SwiftCrossUI | API type SwiftUI sur backends natifs (GTK, etc.) | pas de tag stable examiné | à vérifier | 14 / 14 ; 6 avec >=5 commits ; 322 commits/12 m | macOS/Linux/Windows ; Android naissant [PRESSE] |
| Qt (Quick/QML) | scene graph + RHI (Vulkan/Metal/D3D/GL) ; C++ sans GC | 6.12.0 LTS, 2026-09-30 (tag) ; 6.8.5 LTS le 2026-10-16 | LGPL-3 / GPL / commerciale (LGPL-3.0-only présent dans LICENSES) [DEPOT] | qtbase : 279 / 284 ; 113 avec >=5 commits | desktop, iOS, Android, wasm : officiels [MEMOIRE] ; HarmonyOS ajouté en 6.12 [PRESSE] |
| React Native | Fabric + Hermes (JS) ; vues natives | 0.88.0, 2026-10-15 | MIT [DEPOT] | 411 / 421 ; 78 avec >=5 commits | iOS/Android officiels ; desktop via Microsoft (hors dépôt) ; web via react-native-web |
| Lynx (ByteDance) | moteur propre, JS dual-thread, vues natives | 4.2.0, 2026-10-25 | à vérifier (non lu) | 86 / 88 (dont ci_lynx, 1417 commits, bot) ; 50 avec >=5 commits | iOS/Android/web/desktop selon docs, non revérifié |
| .NET MAUI | contrôles natifs ; pas de Linux officiel | tag stable non extrait | MIT [MEMOIRE] | 115 / (voir §10) ; 46 avec >=5 commits | iOS/Android/macOS(Catalyst)/Windows |
| Uno Platform | WinUI-API ; Skia ou natif ; wasm | tag stable non extrait | Apache-2.0 [MEMOIRE] | 66 ; 43 avec >=5 commits (Martin Zikmund 4803 commits) | iOS/Android/wasm/Win/macOS/Linux |
| Godot (UI) | moteur de jeu, Control nodes | non extrait | MIT [MEMOIRE] | 680 ; 109 avec >=5 commits | toutes, mais pas un toolkit d'app |
| NativeScript | JS vers API natives | non extrait | MIT (corrigé, voir §12) [DEPOT] | 33 (dont github-actions et dependabot) | iOS/Android |

## 2. Méthode de comptage des contributeurs (applicable à toutes les lignes)

- Clone anonyme `git clone --filter=blob:none --no-checkout` sous /home/user/eval/repos/<nom>. Plusieurs de ces clones existaient déjà dans le répertoire à mon arrivée ; leur remote est l'URL GitHub officielle et leur HEAD datait du jour (2026-10-23 à 2026-10-26), donc à jour. Je n'ai pas fait de fetch supplémentaire.
- Commandes : `git shortlog -sn --since=12.months HEAD` (noms distincts) et `git shortlog -sne` normalisé en minuscules (emails distincts) ; `git rev-list --count --since=12.months HEAD` pour les commits ; tags via `git for-each-ref --sort=-creatordate`.
- "Actif" : j'indique aussi le nombre d'auteurs avec >= 5 commits sur 12 mois, seuil arbitraire pour séparer les passages ponctuels du noyau.
- Limites : (a) un même humain avec plusieurs emails/noms gonfle le compte (écart noms/emails visible : Avalonia 179 vs 184, Swift 274 vs 320) ; (b) les bots ne sont pas filtrés dans les totaux : flutter (engine-flutter-autoroll 1972 commits), swift (swift_jenkins 2261, swift-ci 1131), lynx (ci_lynx 1417), NativeScript (github-actions, dependabot) ; (c) les commits fusionnés/squash attribuent à l'auteur du PR ; (d) un dépôt unique ne reflète pas l'écosystème : compose-multiplatform-core (code UI réel), skiko, Kotlin, SkiaSharp, Dart SDK, flutter engine (désormais dans le monorepo flutter/flutter d'après le compte), qt* autres modules ne sont pas comptés ; Qt n'est mesuré que sur qtbase ; (e) le 12 mois est calculé sur la date de commit, pas la date d'auteur.

## 3. Avalonia (.NET, UI en F#)

Faits vérifiés dans le dépôt/registre :
- Version stable la plus récente : 12.1.4 (tag 2026-10-09 ; NuGet api.nuget.org liste 12.0.3 à 12.1.4). 12.0.0-preview1 le 2026-02-17, rc1 le 2026-03-19, 12.0.0 le 2026-04-06. Branche 11.3.x toujours maintenue (11.3.22 le 2026-09-11).
- Licence : MIT, "Copyright (c) AvaloniaUI OÜ" (raw.githubusercontent.com/AvaloniaUI/Avalonia/master/licence.md, consulté 2026-10-25).
- Activité : 895 commits sur 12 mois, dernier commit 2026-10-25 ; 179 noms distincts, 27 avec >= 5 commits. Les 5 premiers : Julien Lebosquain 186, Nikita Tsukanov 85, Emmanuel Hansen 63, Max Katz 42, Benedikt Stebner 41. Noyau réel plus petit que 179 et concentré (une société, AvaloniaUI OÜ, emploie une partie des principaux).

Architecture (preuve : résultats de recherche citant docs.avaloniaui.net et avaloniaui.net, ces hôtes étant bloqués ; non lus directement) :
- Rendu propre (pas de contrôles natifs) sur Skia ; un seul fil UI avec Dispatcher. Runtime .NET avec GC générationnel. FFI : P/Invoke vers C (C-ABI) natif en .NET, bon pour un cœur Rust/Zig exposant des fonctions `extern "C"` ; pas de wrapper requis. [MEMOIRE]
- NativeAOT : la doc officielle (snippet) demande `PublishAot`, `IsAotCompatible`, `x:CompileBindings="True"`, éviter le XAML dynamique, configurer le trimmer pour les contrôles créés dynamiquement, et prévient que certains contrôles tiers ne sont pas compatibles AOT. Le paquet Avalonia.Controls.WebView 12.0.1 est indiqué compatible AOT/trimming.
- Annonce Avalonia 12 (blog officiel, via snippet) : démarrage "1 960 ms -> 460 ms avec NativeAOT" et défilement Android "42 -> 120 FPS". NON VÉRIFIÉ (source : avaloniaui.net/blog/avalonia-12 via WebSearch, 2026-10-25) : chiffres de marketing, appareil, charge et méthode inconnus.
- F# : Avalonia.FuncUI (communautaire, fsprojects) ; aucun support officiel NativeAOT confirmé. Un PR "Try to get the control catalog working in NativeAOT builds" (#423, mai 2024) laisse penser que c'était en cours ; issue non vérifiée. Risque : le trimming/AOT supporte mal la réflexion et FSharp.Core a des cas problématiques [MEMOIRE]. À tester par un prototype avant toute décision.
- Mobile iOS/Android : annoncés en v11 et mis en avant en v12 (navigation par pages, dispatcher Android basé sur Looper/MessageQueue) [OFFICIEL-SNIPPET]. wasm : supporté depuis v11 [OFFICIEL-SNIPPET] ; détails 12.x non trouvés ; taille du bundle wasm (runtime .NET + Skia) : NON VÉRIFIÉ, probablement plusieurs Mo [MEMOIRE].
- Avalonia XPF : produit commercial d'Avalonia pour porter du WPF vers macOS/Linux ; non pertinent pour un projet neuf en F# ; détails de licence non vérifiés [MEMOIRE]. La licence MIT s'applique au cœur ; l'offre commerciale (Avalonia Accelerate/Pro) est séparée.
- Taille typique : NON VÉRIFIÉ, aucune mesure. Un hello world NativeAOT desktop se situe habituellement en dizaines de Mo [MEMOIRE, à mesurer].

Mémoire (comparaison .NET) : MAUI (contrôles natifs, pas de Linux officiel, Hot Reload, 115 contributeurs sur 12 m, 2300 commits, dernier 2026-10-24) et Uno (11 716 commits sur 12 m dont 4803 par un seul auteur ; 66 contributeurs, 43 avec >= 5 commits ; cible wasm, Skia, Linux). Aucune version stable ni licence relue aujourd'hui.

## 4. Kotlin Multiplatform / Compose Multiplatform

Faits [DEPOT] :
- Stable Compose Multiplatform : 1.12.2 (tag 2026-10-15 ; Maven Central compose-gradle-plugin liste 1.12.0/1.12.1/1.12.2) ; 1.12.0 le 2026-08-25 ; prérelease 1.13.0-beta01 le 2026-10-23 (maven-metadata lastUpdated 2026-10-23). Kotlin stdlib stable sur Maven : 2.4.21 (la métadonnée "release" affiche 2.5.0-Beta2).
- Skiko 0.156.0 sur Maven Central (mis à jour 2026-10-23). La 1.12 passe Skia à Milestone 150 [OFFICIEL-SNIPPET, kotlinlang.org].
- Licence Apache-2.0. Dépôt compose-multiplatform : 267 commits sur 12 m, 33 auteurs (13 avec >= 5), top : Nikita Lipsky 43, Konstantin 39, Vladimir Mazunin 23. Dernier commit 2026-10-23. Le moteur UI est dans compose-multiplatform-core : non compté.

Maturité par cible :
- Android : stable (Jetpack Compose, Google).
- iOS : stable depuis 1.8.0 (annonce JetBrains mai 2025, via snippet blog.jetbrains.com / Neowin). 1.12 améliore le défilement des listes paresseuses sur iOS [OFFICIEL-SNIPPET, kotlinlang.org, notes beta].
- Desktop JVM (Windows/macOS/Linux) : stable [MEMOIRE], Hot Reload intégré stable depuis 1.10 [PRESSE]. Démarrage à froid JVM et RAM : NON VÉRIFIÉS. Les options jlink/jpackage distribuent un runtime embarqué ; GraalVM native-image rapporté comme problématique (fil communautaire, ancien).
- Web : Beta depuis Compose Multiplatform 1.9.0 (blog.jetbrains.com, sept. 2025, via snippet). Aucune annonce de passage en stable trouvée à ce jour (recherche 2026-10-25) ; 1.12 apporte le chargement dynamique de sous-ensembles de polices Noto. WasmGC supporté par tous les navigateurs majeurs depuis 2024-12-11 [OFFICIEL-SNIPPET]. Taille bundle wasm : NON VÉRIFIÉ.

Pistes Compose natives sans JVM sur desktop :
- Officiel : aucune. Fils de discussion JetBrains/Slack (anciens) disent que Compose Desktop est JVM seulement, sans plan annoncé ; je n'ai trouvé aucune annonce officielle contraire.
- Réel mais communautaire : "Compose Native" (brahmkshatriya/compose-native, publié sur Maven Central sous dev.brahmkshatriya.compose et listé sur klibs.io). Lu dans le README du dépôt : "Compose Multiplatform for Linux and Windows Kotlin/Native ... no JVM requirement", cibles Linux x64/arm64, Windows x64, macOS x64/arm64 ; s'utilise avec le plugin Compose officiel, Kotlin 2.3.20, Compose 1.13.0-alpha01. Maturité : alpha (tags 1.12.10-alpha02 à alpha04 en août 2026), 72 commits au total, un seul auteur, dernier commit 2026-10-19. Verdict : existe, très jeune, risque bus-factor = 1, pas de preuve de performance ou d'usage en production. Note : le résultat de recherche mentionnait Skiko natif 0.151.5 et aucun macOS ; le README actuel liste macOS (divergence, README retenu comme plus récent).
- FFI : Kotlin/Native cinterop avec des en-têtes C (bon pour un cœur Rust/Zig) ; sur JVM : JNI/Panama (Java 22+ stable) [MEMOIRE]. GC : Kotlin/Native a un GC concurrent depuis 1.9 ; JVM GC classique.

## 5. Flutter

- Dernière stable : 3.47.8, 2026-10-23, Dart SDK 3.13.7 (https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json, consulté 2026-10-25). Tags 3.50.0-0.2.pre le 2026-10-23 (bêta). 3.44 est sorti à Google I/O 2026 [PRESSE].
- Licence BSD-3-Clause ("Copyright 2014 The Flutter Authors", raw.githubusercontent.com/flutter/flutter/master/LICENSE).
- Contributeurs : 5697 commits en 12 m, 411 noms distincts (417 emails), 104 avec >= 5 commits. Bot engine-flutter-autoroll 1972 commits. Humains en tête : Ben Konyi 278, gaaclarke 147. Le dépôt contient désormais le moteur (monorepo), donc le compte couvre framework + moteur ; le SDK Dart est ailleurs.
- Architecture : Impeller (moteur de rendu précompilant les shaders) ; Dart AOT en release, JIT en debug ; GC Dart générationnel ; rendu propre, fils UI + raster séparés. FFI : dart:ffi vers C (OK pour Rust/Zig), package flutter_rust_bridge communautaire [MEMOIRE].
- Impeller/Skia : la feuille de route 2026 (flutter.dev/blog/flutter-darts-2026-roadmap, via snippet) dit vouloir supprimer le backend Skia sur Android 10+ et faire de Wasm le défaut web, "énoncé d'intention, pas garantie". Un article de presse affirme que 3.44 a supprimé Skia sur Android 10+ ; un autre dit que l'opt-out est seulement déprécié depuis 3.38 : sources conflictuelles, NON VÉRIFIÉ.
- Web : CanvasKit/Skwasm (WasmGC). Défaut wasm : objectif seulement, NON VÉRIFIÉ. DevTools utilise wasm par défaut en 3.44 [OFFICIEL-SNIPPET].
- Plateformes : iOS, Android, macOS, Linux (GTK), Windows, web ; tout officiel [MEMOIRE]. Port HarmonyOS mentionné par un article tiers.
- Tailles et RAM : NON VÉRIFIÉES (non mesurées).
- Langage unique : Dart ; l'UI en Dart, cœur Rust via FFI.

## 6. Swift

- Dernière stable : swift-6.4.1-RELEASE (tag 2026-10-07) ; 6.4.0 le 2026-09-13 ; 6.3.3 le 2026-06-25 ; 6.3 le 2026-03-20 [DEPOT, swiftlang/swift]. Dépôt : 16 272 commits/12 m (dont ~3 400 par deux bots), 274 noms distincts / 320 emails, 137 avec >= 5 commits ; développeurs en tête : Slava Pestov 688, Doug Gregor 624, Saleem Abdulrasool 563.
- SDK Android officiel : livré avec Swift 6.3, annonce rapportée le 2026-03-24 par plusieurs médias (9to5Google 2026-03-28, I-Programmer, AlternativeTo) ; Skip.dev confirme "maintained, tested, released by the Swift project" (source partie prenante). Je n'ai pas lu swift.org (bloqué). Un blog tiers contredit (nightlies seulement) : traité avec prudence. Limites : bibliothèques de base (Foundation, Dispatch...) ; SwiftUI non porté ; interop Java/Kotlin via swift-java ; installation `swift sdk install <URL>`. "Officiel" ne signifie pas "plateforme supportée au même niveau" (commentaire tiers).
- Swift sur Linux : supporté de longue date ; SDK Static Linux et WebAssembly listés sur les pages d'installation swift.org (snippet, branche release/6.3 du SDK WASM le 2026-05-05). Windows : toolchain existe [MEMOIRE], non confirmé par les recherches du jour.
- UI cross-platform :
  - SwiftUI : Apple uniquement (iOS, macOS, etc.).
  - Skip : Skip Lite (transpile en Kotlin) ou Skip Fuse (compile Swift natif via le SDK officiel) ; iOS = SwiftUI, Android = Jetpack Compose. Dépôt skip : 8 contributeurs, 137 commits/12 m, 1.9.19 le 2026-10-23. Gratuit pour indies depuis 1.6.28 (blog Skip, source partie prenante). Open source annoncé en janvier 2026 (InfoQ 2026-01).
  - SwiftCrossUI : API type SwiftUI sur macOS/Linux/Windows, Android naissant (InfoQ, janv. 2026) ; dépôt : 322 commits/12 m, 14 auteurs, 6 avec >= 5 commits, un auteur dominant (stackotter 184). Dernier commit 2026-09-21. Maturité : communautaire/expérimentale.
- Performance : Swift est AOT sans GC (ARC), FFI C direct ; un cœur Rust/Zig en C-ABI s'intègre bien. Aucune mesure.

## 7. Qt (Qt Quick/QML)

- Dernière version : 6.12.0 (LTS, tag 2026-09-30 ; Phoronix 2026-09-30) ; branches : 6.11.2 le 2026-08-18, 6.8.5 LTS le 2026-10-16, 6.5.10 LTS le 2026-09-03 [DEPOT]. Nouveautés 6.12 (via snippets Phoronix/Linuxiac/doc.qt.io) : QML Hot Reload dans le moteur, StyleKit (Qt Labs), Canvas Painter (RHI) entièrement supporté, QRhi indirect rendering, support HarmonyOS.
- Licence : LGPL-3.0-only présente dans LICENSES du dépôt [DEPOT] ; double licence LGPL/GPL/commerciale [MEMOIRE] (certains modules GPL seulement, à vérifier module par module). Contraintes LGPL : lien dynamique sur mobile/iOS à vérifier juridiquement (non traité).
- Contributeurs qtbase : 5859 commits/12 m, 279 noms / 284 emails, 113 avec >= 5 commits ; Marc Mutz 584, Assam Boudjelthia 378, Thiago Macieira 332. Autres modules (qtdeclarative, qtquick3d) non comptés.
- Architecture : scene graph threadé (thread de rendu séparé), RHI (Vulkan/Metal/D3D11-12/OpenGL), moteur QML avec JS (V4) ; cœur C++ sans GC ; qmlsc/QML compilé. FFI : C++ ; liaisons Rust (cxx-qt, KDAB) communautaires/commerciales [MEMOIRE]. Plateformes : desktop, iOS, Android, wasm (Emscripten) officiels [MEMOIRE]. Tailles/RAM wasm : NON VÉRIFIÉ.

## 8. Autres

- React Native : 0.88.0 le 2026-10-15 ; MIT ; 3406 commits/12 m ; 411 noms ; 78 avec >= 5 commits. Fabric + Hermes (JS bytecode, GC). Nouvelle architecture : défaut depuis 0.76 [MEMOIRE].
- Lynx : 4.2.0 le 2026-10-25 ; 86 contributeurs, 50 avec >= 5 commits ; licence non lue. Maturité hors Chine mal documentée ; non revérifié.
- Godot : UI Control, 680 contributeurs/12 m, MIT ; pas conçu comme toolkit d'application. Unity : source fermée, non pertinent.
- NativeScript : 33 auteurs dont bots, 499 commits. Capacitor/Tauri/Electron : webview ; Tauri (Rust) et Electron mémoire seulement, non analysés.

## 9. Lecture pour le critère PERFORMANCE (sans mesure)

Hypothèses plausibles, non mesurées : Qt (C++ natif, sans GC) et Swift natif/Compose Native probablement les plus économes en RAM et démarrage à froid ; Avalonia NativeAOT supprime le JIT mais garde un GC et un rendu Skia ; Flutter AOT/Impeller bien documenté pour la fluidité ; Compose JVM desktop paie le démarrage JVM. Ces points sont des hypothèses [MEMOIRE], à confirmer par un banc de test commun (hello world + liste de 10 000 lignes sur la même machine) avant toute décision.

## 10. Sources (URL, date de consultation 2026-10-25 sauf mention)

Dépôts clonés (tags, commits, auteurs) : https://github.com/AvaloniaUI/Avalonia, JetBrains/compose-multiplatform, flutter/flutter, swiftlang/swift, skiptools/skip, qt/qtbase, facebook/react-native, lynx-family/lynx, stackotter/swift-cross-ui, unoplatform/uno, dotnet/maui, godotengine/godot, NativeScript/NativeScript, brahmkshatriya/compose-native (clone git seulement ; pages web GitHub bloquées).
Fichiers bruts : https://raw.githubusercontent.com/AvaloniaUI/Avalonia/master/licence.md ; https://raw.githubusercontent.com/JetBrains/compose-multiplatform/master/LICENSE.txt et CHANGELOG.md ; https://raw.githubusercontent.com/flutter/flutter/master/LICENSE ; https://raw.githubusercontent.com/qt/qtbase/dev/LICENSES/LGPL-3.0-only.txt ; https://raw.githubusercontent.com/facebook/react-native/main/LICENSE.
Registres : https://api.nuget.org/v3-flatcontainer/avalonia/index.json ; https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json ; https://repo.maven.apache.org/maven2/org/jetbrains/compose/compose-gradle-plugin/maven-metadata.xml, .../kotlin-stdlib/maven-metadata.xml, .../skiko/skiko/maven-metadata.xml.
Recherches web (résultats seulement, pages non lues sauf mention) : https://avaloniaui.net/blog/avalonia-12 ; https://docs.avaloniaui.net/docs/deployment/native-aot (WebFetch échoue : hôte non résolu/bloqué) ; https://blog.jetbrains.com/kotlin/2025/09/compose-multiplatform-1-9-0-compose-for-web-beta/ ; https://blog.jetbrains.com/kotlin/2025/05/compose-multiplatform-1-8-0-released-compose-multiplatform-for-ios-is-stable-and-production-ready/ ; https://blog.jetbrains.com/kotlin/2026/08/compose-multiplatform-1-12-0/ ; https://kotlinlang.org/docs/multiplatform/whats-new-compose-112.html ; https://klibs.io/project/brahmkshatriya/compose-native ; https://flutter.dev/blog/flutter-darts-2026-roadmap ; https://flutter.dev/blog/whats-new-in-flutter-3-44 ; https://doc.qt.io/qt-6/whatsnew612.html ; https://www.phoronix.com/news/Qt-6.12-LTS-Released ; https://9to5google.com/2026/03/28/swift-a-coding-language-developed-by-apple-now-offers-official-android-support/ ; https://skip.dev/blog/swift-63-android-support/ ; https://www.infoq.com/news/2026/01/swift-skip-open-sourced ; https://www.i-programmer.info/news/98-languages/18766-swift-63-adds-android-sdk.html.

## 11. Points non vérifiés

1. Toutes les mesures de perf, RAM, taille binaire, démarrage à froid : aucune faite. Les chiffres Avalonia (1 960 -> 460 ms, 42 -> 120 FPS) sont du marketing non reproduit.
2. NativeAOT + trimming + F# (FuncUI) avec Avalonia 12 : non testé, support officiel non confirmé.
3. Statut exact des cibles Avalonia sur iOS, Android et wasm en 12.x (stable vs preview) : non lu faute d'accès à avaloniaui.net.
4. Web Compose Multiplatform : Beta confirmé en 1.9 ; état en 1.12/1.13 non confirmé.
5. Swift 6.3 Android : source primaire swift.org non lue ; un blog contradictoire existe.
6. Skia retiré ou non sur Android dans Flutter 3.44+ : sources conflictuelles. Wasm défaut web Flutter : objectif.
7. Licences non lues : Skip, Lynx, SwiftCrossUI, Swift (exception runtime) ; MAUI, Uno, Godot, NativeScript de mémoire. Versions stables MAUI, Uno, Godot, NativeScript non extraites.
8. Avalonia XPF : détails commerciaux non vérifiés.
9. Contributeurs : voir limites de la section 2 (bots, emails multiples, dépôts satellites non comptés, Qt = qtbase seulement).
10. Plateformes Swift Windows/wasm UI, Lynx desktop, Qt wasm/iOS tailles : non vérifiés aujourd'hui.

## 12. Mises à jour après réouverture des hôtes (consultation 2026-10-25)

Note de méthode : WebFetch ne résolvait toujours pas docs.avaloniaui.net, avaloniaui.net, www.jetbrains.com, docs.flutter.dev, doc.qt.io ; j'ai donc utilisé curl (--compressed) puis extrait le texte des pages. swift.org répond via WebFetch/curl. Aucun build, aucune mesure.

### Avalonia
- CONFIRMÉ (https://docs.avaloniaui.net/docs/deployment/native-aot, page "last updated May 7, 2026") : `PublishAot` sur l'exécutable, `IsAotCompatible` sur toutes les bibliothèques, `x:CompileBindings="True"`, pas de XAML dynamique, DI à la compilation, `TrimmerRootAssembly` pour les erreurs de réflexion ; limites : contrôles dynamiques à déclarer au trimmer, certains contrôles tiers non AOT-compatibles, aperçu design-time limité. La page vante démarrage plus rapide et empreinte mémoire réduite : affirmations qualitatives du fournisseur, sans chiffre, NON VÉRIFIÉES. XPF supporte aussi Native AOT (même page).
- CONFIRMÉ (https://avaloniaui.net/blog/avalonia-12, daté 7 avril 2026, Steven Kirk) : 12 sorti ; compiled bindings par défaut ; ciblage .NET 10 et SkiaSharp 3.0 ; Direct2D1, Tizen et Avalonia.Browser.Blazor supprimés ; WebView entièrement open source ; 19 employés, financement Devolutions sur 3 ans. Chiffres du billet, tous NON VÉRIFIÉS (source avaloniaui.net/blog/avalonia-12, 2026-10-25, marketing) : Android démarrage 1 960 ms -> 460 ms avec NativeAOT, défilement 42 -> 120 FPS, CPU au repos 0,20 -> <0,01 %, "+1 867 % de FPS" sur une scène de 350 000 éléments. Le billet annonce aussi un partenariat avec l'équipe Flutter pour un moteur Impeller pour .NET : annonce fournisseur, non vérifiée.
- CONFIRMÉ (https://docs.avaloniaui.net/docs/overview/supported-platforms) : cibles Windows, macOS, Linux, iOS, Android, WebAssembly, avec niveaux (Tier 1 testé, Tier 2 best effort, Tier 3 commercial). Tier 1 : Windows 11 24H2, macOS 26, iOS/iPadOS 26, Android 16 (API 36), Ubuntu 25.x/Fedora 43/Debian 13, Raspberry Pi OS embarqué. Mobile exige .NET 10. Rendu Linux : OpenGL via Skia (X11), EGL sous Wayland, logiciel en framebuffer. Backend Wayland natif en paquet optionnel depuis 12.1.0 (infirme "Wayland absent"). Skia construit contre glibc 2.17, musl = build Skia custom (Tier 3). Lacune de la page : le statut stable/preview de iOS, Android et wasm n'est pas dit au-delà des tiers.
- wasm (https://docs.avaloniaui.net/docs/deployment/webassembly) : le bundle contient le runtime .NET et les assemblies ; la doc dit que le téléchargement initial "peut être grand" et recommande trimming, compression Brotli (-60 à 70 % sur .wasm, affirmation doc), AOT. Taille réelle : NON VÉRIFIÉE.
- XPF (https://docs.avaloniaui.net/xpf) : produit commercial qui remplace MilCore de WPF par le moteur de rendu Avalonia, compatibilité binaire avec les contrôles tiers (Telerik, DevExpress...), mode Hybrid XPF. Tarif/licence exacts : non lus (page pricing non consultée). Utilité pour un projet neuf F# : nulle.
- Avalonia.FuncUI : api.nuget.org confirme 2.0.0 publié le 2026-07-28, licence MIT, dépend d'Avalonia 12.1.0 et FSharp.Core 6.0.0. Support NativeAOT : TOUJOURS NON VÉRIFIÉ (aucune page ni issue accessible hors github.com ne le dit ; le moteur de recherche n'a rien renvoyé de spécifique). Le point reste à tester par prototype.
- Licence : nuspec Avalonia 12.1.4 = MIT (api.nuget.org), confirmant le dépôt.

### Compose Multiplatform
- CONFIRMÉ (https://www.jetbrains.com/help/kotlin-multiplatform-dev/supported-platforms.html et https://kotlinlang.org/docs/multiplatform/supported-platforms.html) : Compose Multiplatform = Android Stable, iOS Stable, Desktop (JVM) Stable, Web (Kotlin/Wasm) Beta ; Kotlin/JS Web stable au niveau Kotlin seulement. Réserve : le pied de page de ces pages porte la date "10 September 2025", donc la page peut être en retard sur 1.12/1.13.
- https://kotlinlang.org/docs/multiplatform/whats-new-compose-112.html (page titrée 1.12.2) : aucune mention d'un passage de Web en stable ; nouveautés web = police de repli automatique (Noto à la demande). Donc, à ce jour, Web = Beta d'après les docs officielles ; stable infirmé/non annoncé.
- Compose natif sans JVM : aucune cible officielle dans les tableaux ci-dessus (confirmé par absence). Seule piste trouvée : brahmkshatriya/compose-native (alpha, 1 auteur), inchangé.

### Flutter
- CONFIRMÉ (https://docs.flutter.dev/perf/impeller) : Impeller est le seul moteur sur iOS ; Android API 29+ par défaut avec repli sur le moteur OpenGL "legacy" si Vulkan indisponible ou API < 29, et opt-out toujours documenté (`--no-enable-impeller`, `EnableImpeller=false`) ; macOS, Linux et Windows : Impeller activé par défaut depuis Flutter 3.47, opt-out à supprimer dans une version future ; Web : "utilise actuellement Skia, pourrait utiliser Impeller plus tard". INFIRMÉ : l'affirmation de presse "Skia supprimé sans opt-out sur Android 10+" n'est pas ce que dit la doc officielle consultée aujourd'hui.
- Wasm (https://docs.flutter.dev/platform-integration/web/wasm) : exige Flutter 3.24+ ; `flutter build web --wasm` produit wasm plus un repli dart2js+CanvasKit ; Flutter 3.50 : Chrome/Chromium 137+ exécute dart2wasm + skwasm par défaut avec --wasm, Firefox 147+ via `wasmAllowList.gecko`, Safari <= 26.1 retombe sur CanvasKit ; chargement différé wasm en 3.50 ; `--no-strip-wasm` ajoute environ 47 % (chiffre doc). Wasm défaut web : toujours un choix de build (--wasm), pas confirmé comme défaut global.
- Taille minimale (https://docs.flutter.dev/perf/app-size) : la page explique comment mesurer ; elle ne fournit pas de chiffre minimal exploitable dans mes extraits. NON VÉRIFIÉ.

### Swift
- CONFIRMÉ (https://www.swift.org/blog/swift-6.3-released/, daté 24 mars 2026 ; https://www.swift.org/documentation/articles/swift-sdk-for-android-getting-started.html) : Swift 6.3 inclut "la première version officielle du Swift SDK for Android" ; le guide installe le SDK 6.4.1 via `swift sdk install https://download.swift.org/swift-6.4.1-release/android-sdk/...` (download.swift.org répond pour ce chemin), exige la toolchain open source de même version et le NDK Android LTS r30. Le guide dit que Swift prend en charge Linux, Windows et systèmes embarqués. Le billet 6.3 ne mentionne pas WebAssembly/Static Linux (infirme mon snippet sur ce point précis pour cette page). Le blog contradictoire "nightlies seulement" est infirmé.
- SwiftUI/Skip/SwiftCrossUI : SwiftUI Apple seulement : inchangé. Skip : LICENSE.TXT du dépôt = Mozilla Public License 2.0 (Copyright 2026 Skip) [DEPOT]. SwiftCrossUI : LICENSE = MIT (Copyright 2022 stackotter) [DEPOT]. Swift : Apache-2.0 [DEPOT] (exception runtime non revérifiée).

### Qt
- https://www.qt.io/licensing : Community Edition sous LGPL ou GPL ; édition commerciale distincte (confirmé). Détail module par module (modules GPL seuls) : NON VÉRIFIÉ.
- https://doc.qt.io/qt-6/whatsnew612.html : Android passe à Gradle 9.5.1/AGP 9.2.1, target SDK API 37, multi-module Gradle ; iOS : changements mineurs ; WebAssembly : QAudioSource/Sink via AudioWorklets, vidéo vers WebGL en copie zéro pour Qt Quick ; HarmonyOS ajouté ; CMake 3.25 requis ; Qt Canvas Painter sorti de Technology Preview. Tailles wasm : NON VÉRIFIÉ.
- QtQuick.Templates : non traité sur cette page (aucune mention trouvée) ; reste NON VÉRIFIÉ.

### Licences "de mémoire" de la section 11, désormais lues
- MAUI : MIT (Copyright .NET Foundation and Contributors) CONFIRMÉ [DEPOT]. Uno : Apache-2.0 CONFIRMÉ [DEPOT]. Godot : fichier LICENSE.txt de la branche master (Copyright Godot Engine contributors), type MIT non reconfirmé ligne à ligne [DEPOT partiel]. NativeScript : fichier MIT (OpenJS Foundation) : INFIRME ma mention Apache-2.0. Lynx : Apache-2.0 CONFIRMÉ [DEPOT]. Skip : MPL-2.0 [DEPOT]. SwiftCrossUI : MIT [DEPOT].
- Toujours non vérifiés : versions stables MAUI/Uno/Godot/NativeScript, détails commerciaux XPF, tout chiffre de perf/taille/RAM (aucune mesure).

Sources additionnelles (consultées 2026-10-25) : les URL citées dans cette section ; https://raw.githubusercontent.com/... pour les LICENSE (godot, NativeScript, Lynx, Swift) ; fichiers LICENSE lus dans les clones git locaux (maui, uno, skip, swift-cross-ui) ; https://api.nuget.org/v3-flatcontainer/avalonia.funcui/ et /avalonia/12.1.4/avalonia.nuspec.
