# Stack compose : Kotlin / Compose Multiplatform Desktop (JVM + Skiko), rendu logiciel

## [HISTORIQUE] Version 1.5.12 imposee par le blocage reseau initial
La derniere stable (1.12.2, Kotlin 2.4.21 ; plugins `org.jetbrains.compose` et `org.jetbrains.kotlin.plugin.compose` se resolvent bien
sur plugins.gradle.org / Maven Central) NE PEUT PAS etre resolue : toutes les versions >= 1.6.0 dependent (jar JVM) d'artefacts
`androidx.*` (annotation, collection, lifecycle-*, savedstate, compose.runtime...) qui n'existent que sur Google Maven.
maven.google.com repond 301 vers `dl.google.com/dl/android/maven2/...` (BLOQUE, 403) et ces artefacts sont absents de Maven Central (404).
Erreur exacte (logs/build-probe.log, 1.12.2) :
`Could not resolve androidx.lifecycle:lifecycle-common:2.9.4 ... Could not GET 'https://dl.google.com/dl/android/maven2/androidx/lifecycle/lifecycle-common/2.9.4/lifecycle-common-2.9.4.pom'. Received status code 403 from server: Forbidden`
(idem androidx.annotation, androidx.collection, androidx.savedstate, androidx.compose.runtime:runtime:1.12.1...).
Contournement "exclure androidx.*" teste (1.8.2) : build OK mais crash au lancement `NoClassDefFoundError: androidx/collection/MutableObjectList`
(les org.jetbrains.compose.collection-internal:* ne sont que des stubs javadoc). Teste : 1.12.2, 1.11.1, 1.10.3, 1.9.3, 1.8.2, 1.7.3, 1.6.2, 1.6.0 -> tous KO.
CMP 1.5.12 (Compose 1.5.x, Kotlin 1.9.22) est la plus recente dont le graphe est 100 % Maven Central + plugin portal : build et run OK.
Consequence : les chiffres Compose refletent un runtime Compose/Skiko de fin 2023 (skiko 0.7.85.4), plus ancien que la baseline KMP actuelle
(les versions recentes ont de meilleures perfs, p.ex. LazyList prefetch, scrolling, etc.). A re-mesurer si dl.google.com est debloque (changer 2 numeros de version).
Pas de plugin `kotlin.plugin.compose` (n'existe que Kotlin >= 2.0, et CMP 1.5.12 exige Kotlin 1.9.x) : compilateur Compose JetBrains 1.5.x integre au plugin CMP.

# MISE A JOUR (cible principale) : Compose Multiplatform 1.12.2 / Skiko 0.150.2 / Kotlin 2.4.21
Apres deblocage de dl.google.com / maven.google.com par le coordinateur, la stack est reconstruite avec la derniere stable :
plugins `org.jetbrains.compose` 1.12.2 + `org.jetbrains.kotlin.plugin.compose` 2.4.21, Kotlin 2.4.21, coroutines 1.10.2, repo `google()` + mavenCentral. Resolu : ui-desktop 1.12.2, runtime-desktop 1.12.2, skiko-awt 0.150.2, kotlin-stdlib 2.4.21.
Code source inchange (815 lignes Kotlin), compile sans modification. Build : `build.sh` ; a froid ~1 min 30 (logs/build-cmp1.12.2.log). Erreurs rencontrees : HTTP 429 "Too Many Requests" transitoires de repo.maven.apache.org
(gradle-plugin-internal-jdk-version-probe:1.12.2, skiko-awt-runtime-linux-x64:0.150.2) -> relancer le build suffit.
- Rendu logiciel : `SKIKO_RENDER_API=SOFTWARE_FAST` (ou -Dskiko.renderApi, dans app.cfg) fonctionne ; `SOFTWARE_COMPAT` fonctionne mais ~6x plus lent (30 fps en liste) ; `OPENGL` -> "[SKIKO] warn: Fallback to next API / RenderException: Cannot create Linux GL context" puis repli logiciel (~158 fps).
- FFM : JDK 21 (preview) conserve, `--enable-preview` ; pas de JDK >= 22 disponible localement (non cherche via toolchains). 21.9 M triplets/s.
- 9 scenarios OK (fumee 8 s) : sum_heights 8799584 sur list/stream/stress/idle. text : select-all toujours non supporte (LazyColumn+SelectionContainer). nav : 20 transitions en 8 s.
- Tailles : dist 134 Mo (runtime jlink ~85 Mo ; jars app ~49 Mo, libskiko ~ voir jar), uber-jar 33 Mo. Ancienne build 1.5.12 conservee dans `build-cmp1.5/` (dist/ 130 Mo, uber-jar, build.gradle.kts.cmp1.5, run.sh.cmp1.5).
- Commande harnais inchangee : `/home/user/eval/stacks/compose/run.sh` (BENCH_VARIANT=tuned, BENCH_MODE=jar possibles).
- Fumee comparee (6 s, CPU partage, indicatif) : list 1.5.12 ~138 fps (p50 6.6 ms) -> 1.12.2 ~182-190 fps (p50 4.5 ms) ; tuned 209 fps ; canvas 5000 : 2.0 fps (540 ms) -> 5.0 fps (198 ms) ; canvas 20000 : 0.6 -> 1.5 fps ;
  idle RSS 124 Mo -> 182 Mo ; list RSS max 250 -> 305 Mo ; stream 10k 123 -> 198 fps ; text 56 -> 123 fps ; nav 176 -> 247 fps ; core 8-10 -> 26 fps ; stress ~1 -> 2.9 fps ;
  **images regresse : 107 -> 28 fps** (cause non investiguee ; probable cout du dessin/zoom des ImageBitmap en 1.12) ; demarrage JVM->1re frame ~1.0-1.3 s (inchange).

# ANCIENNE SECTION (CMP 1.5.12, historique, dl.google.com etait bloque)
## Versions
- JDK systeme : OpenJDK 21.0.12.1 (Ubuntu). **FFM = preview en 21** -> `--enable-preview --enable-native-access=ALL-UNNAMED` (pas de JNA/JNI necessaire). Kotlin 1.9.22 appelle java.lang.foreign sans probleme (compile et run avec le flag).
- Gradle 8.14.3 (`--no-daemon --max-workers=2`), Kotlin 1.9.22 (jvmToolchain 21), Compose Multiplatform 1.5.12 (compose.desktop.linux_x64 + foundation), Skiko 0.7.85.4, kotlinx-coroutines 1.9.0 (core + swing).
- Depots : mavenCentral, gradlePluginPortal, `maven("https://maven.google.com")` (inutile en pratique, redirige vers dl.google.com).

## Build / lancement
- Build : `/home/user/eval/stacks/compose/build.sh` (createDistributable + packageUberJarForCurrentOS). Compile : ~40 s (Kotlin) ; build complet a froid ~1 min 20 s ; distribution seule ~40 s (logs/build-*.log).
- **Commande harnais** : `/home/user/eval/stacks/compose/run.sh` (ajouter --scenario ... etc.). Par defaut execute `build/compose/binaries/main/app/app/bin/app` (lanceur jpackage + runtime jlink, JVM par defaut = G1, heap max = 1/4 RAM).
  Les options JVM sont dans `lib/app/app.cfg` : `-Dskiko.renderApi=SOFTWARE_FAST --enable-preview --enable-native-access=ALL-UNNAMED`.
- Variantes : `BENCH_VARIANT=tuned` = `-Xms512m -Xmx1g -XX:+UseSerialGC -XX:-UsePerfData -Xshare:auto` (injectes via JAVA_TOOL_OPTIONS car le runtime jlink de jpackage n'a pas de bin/java) ;
  `BENCH_MODE=jar` = uber-jar + JDK systeme (`java -cp ... bench.MainKt`). Harnais : `--env BENCH_VARIANT=tuned`.
- Variable de rendu qui marche : `SKIKO_RENDER_API=SOFTWARE_FAST` (aussi `-Dskiko.renderApi=SOFTWARE_FAST`, deja dans app.cfg). Sans cela / avec `OPENGL` : `[SKIKO] warn: Fallback to next API ... RenderException: Cannot create Linux GL context` puis repli logiciel (meme perfs). `SOFTWARE` (compat) marche aussi. Pas de plafond vsync observe (>100 fps en liste).
- Le core C-ABI est charge par chemin absolu `/home/user/eval/core/target/release/libbenchcore.so` (Core.kt, `SymbolLookup.libraryLookup`).

## Implementation (815 lignes Kotlin, src/main/kotlin/bench/*.kt)
Boucle de frames : `LaunchedEffect` + `withFrameNanos` en boucle continue ; `frame_ms` = intervalle entre callbacks consecutifs (System.nanoTime au debut du callback ; Compose Desktop ne lance la frame suivante qu'apres le rendu Skiko de la precedente). Warm-up 2 s executant le meme script puis `reset()` et mesure. Fenetre non decoree 1280x720 (barre de titre dessinee par l'app, pas de WM sous Xvfb).
- idle : OK (shell + 1re page, aucun redessin, dort 1 s + duration).
- list : OK. LazyColumn, hauteurs variables 48..128 dp, sum_heights = 8799584 verifie. Defilement via `ScrollableState.scroll { scrollBy(v(t)*dt) }`, final_offset = px consommes. `rows_drawn` = nb d'executions de drawBehind des lignes (Compose cache les couches : ne vaut PAS "lignes dessinees par frame", ~320 sur 6 s).
- stream : OK. Producteur coroutine Dispatchers.Default, xorshift32 seed 0x2545F491, Channel(100_000, DROP_LATEST), application par lots a chaque frame ; un MutableDoubleState par ligne (cree a la demande) -> seules les lignes visibles se recomposent. 10000/s : generated 59382 / applied 59268 / dropped 0 sur 6 s.
- canvas : OK mais tres lent (voir fumee). Un seul `Canvas`, pinceaux Brush.linearGradient pre-crees (coord. locales, `translate`), ombre portee via Skia natif (`ImageFilter.makeDropShadowOnly`, sigma 4) pour 1 forme / 10 (pas d'API ombre dans DrawScope), clip `clipPath` rect arrondi 1000x600, graphe Path 10 000 lineTo reconstruit par frame (zone 1000x100 en bas). Rebond dans la fenetre 1280x720 (le clip n'en montre qu'une partie). Profil jstack : ~90 % du temps dans `Canvas._nDrawPicture` natif (rastérisation Skia logicielle), pas dans le code Kotlin.
- core : OK. FFM downcalls (`MethodHandle.invoke`, pas invokeExact depuis Kotlin) validate+parse+hash sur la trame A5 01 05 "hello" ; thread Dispatchers.Default pour core_heavy(100)/s ; 1 000 formes (sans ombre) animees. ~15-22 M triplets/s.
- stress : OK (list 880 px + stream 5000/s + canvas 400x720 5000 formes + core) ; echantillons `managed_heap_kb` toutes les 10 s dans ops.heap_samples (chaine JSON).
- text : PARTIEL. 1 Mo coupe en 9 599 paragraphes dans un LazyColumn sous `SelectionContainer` (un seul Text de 1 Mo depasse la contrainte max Compose 2^18 px). Defilement aller-retour 3000 px/s OK. **« Tout selectionner » a t=5 s NON supporte** (pas d'API sur un LazyColumn) -> note dans ops.select_all. `time_to_first_display_ms` = depuis le demarrage JVM ; `layout_ms` = composition->2e frame (1re page seulement, lazy). Bidi/emoji/CJK s'affichent (polices de repli Skia), non verifie de pres.
- images : OK. LazyVerticalGrid 9 colonnes de 128 dp ; decodage PNG (Skia) + reduction 128x128 sur Dispatchers.IO, cache ; defilement aller-retour 3000 px/s + zoom 1->3->1 / 6 s (graphicsLayer) + pan. 600 images decodees, decode_total_ms ~5400 (cumule, plusieurs threads).
- nav : OK. AnimatedContent (slide 300 ms) 3 ecrans, modale AnimatedVisibility fondu 200 ms, boucle de calcul sur Dispatchers.Default ; les transitions continuent jusqu'a la fin de duration (13 en 6 s ; 20 atteintes avec duration >= ~10 s).
- Ecarts : IME non traitee ; text select-all non supporte ; rows_drawn semantique differente ; nav/images : boucle sur toute la duree.

## Fumee (Xvfb 1280x720, 6 s, machine partagee avec d'autres agents -> indicatif uniquement)
list ~120-145 fps (p50 6.6 ms, p99 12-25 ms) ; stream 10k/s ~123 fps ; text ~56 fps ; images ~107 fps ; nav ~176 fps (anim) ;
canvas 1000 formes ~8 fps (125 ms/frame), **5000 formes ~2 fps (540-560 ms/frame)**, 20000 formes ~0.6 fps (2 s/frame) ; core (1000 formes + 2 threads FFM) ~8-10 fps ; stress ~1 fps (frames ~920 ms).
Variantes JVM (list) : defaut/dist 138 fps, tuned/dist 140 fps, jar defaut 144 fps : difference negligeable ; RSS max : defaut dist ~250 Mo, jar ~215 Mo, tuned ~270 Mo (Xms512m) ; idle ~124 Mo ; stress ~450 Mo.
Demarrage (fumee) : JVM->main ~60-120 ms ; JVM -> 1re frame ~0.95-1.1 s (a froid sous charge ~1.8 s).

## Taille / dependances d'execution
- Distribution `build/compose/binaries/main/app/app` : 130 Mo (runtime jlink 85 Mo dont `lib/modules` 56.8 Mo ; jars app 45 Mo dont skiko natif 29.6 Mo dans le jar skiko-awt-runtime, extrait dans ~/.skiko/ au 1er lancement). Uber-jar : 28 Mo (+ JDK systeme ~300 Mo non compte). Lanceur `libapplauncher.so` 204 Ko.
- `ldd libskiko-linux-x64.so` (29.6 Mo) : libGL, libX11, libfontconfig, libm, libc, libGLdispatch, libGLX, libxcb, libfreetype, libexpat, libXau, libXdmcp, libz, libbz2, libpng16, libbrotlidec/common, libbsd, libmd. `libjvm.so` : libstdc++, libm, libgcc_s, libc. AWT/X11 : libawt_xawt etc. du runtime jlink.
- libbenchcore.so : 411 Ko.

## Problemes rencontres
- google() Gradle -> dl.google.com 403 (ci-dessus). `kotlin.plugin.compose` 2.4.21 + CMP 1.12.2 : plugins OK, dependances KO. CMP 1.6.2 + Kotlin 2.0.21 : `Compose Multiplatform 1.6.2 doesn't support Kotlin 2.0.21` (logs/build-probe-1.6.2.log, premier essai).
- Runtime jlink de jpackage sans `bin/java` (exec 127 la 1re fois) -> variante tuned via JAVA_TOOL_OPTIONS.
- `ScrollableState.scrollBy` absent de CMP 1.5.12 (remplace par `scroll { scrollBy }`) ; `Pixmap.scalePixels` sans param cache.
- Incident : un `pkill Xvfb` de ma part pendant le debut des essais a pu couper des serveurs X d'autres agents (display :71 partage) ; mes essais ulterieurs utilisent des displays aleatoires :200-699 et ne tuent que leur propre Xvfb. Les chiffres de fumee ont ete pris sur CPU partage.
