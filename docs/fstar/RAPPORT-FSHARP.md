# Porte 1 : F* → F# → .NET 10 → NativeAOT — passe 2 (dernières versions), rapport mesuré

Date : 2026-10-26. Tout ce qui est marqué OK a été exécuté. Logs bruts : `fstar/logs/v2/` (passe 1 archivée dans `fstar/logs/passe1/`).
Reproduction : `fstar/build.sh` (sorties de build dans `$BUILD=/opt/krml/p1/build`, hors dépôt) ; mesures : `fstar/measure.py`.

## Versions exactes (vérifiées le 2026-10-26, `logs/v2/01-versions.log`)
| Élément | Version | Source de la vérification |
|---|---|---|
| F* | **2026.10.25**, commit `362e792b`, binaire construit avec OCaml 5.3.0 (`/opt/fstar-2026.10.25/fstar`) | `git ls-remote --tags https://github.com/FStarLang/FStar` : dernier tag `v2026.10.25` (aucun plus récent) |
| Z3 | **5.3.0** (livré dans la release, `lib/fstar/z3-5.3.0`) | défaut de cette version : `FStarC.Options.fst:331 ("z3version", String "5.3.0")`, `fstar.exe --help` « Default: 5.3.0 », `--locate_z3 5.3.0` → le binaire livré ; CHANGES.md « The default Z3 version is now 5.3.0 ». La release livre aussi 4.8.5, 4.13.3 et 4.15.3 (non utilisés) |
| .NET SDK | **10.0.403** (`/opt/dotnet10`), runtime 10.0.14 | `builds.dotnet.microsoft.com/.../10.0/releases.json` : latest-sdk 10.0.403, latest-release 10.0.14 (2026-10-25) |
| FSharp.Core | 10.1.403 (implicite, livré avec le SDK) | NuGet : 11.0.100 existe (stable) → **variante testée OK** (9201/0, binaire 2 352 864 o, `logs/v2/53`) ; je garde celui du SDK par défaut |
| ILCompiler (NativeAOT) | 10.0.14 | NuGet flat container : dernière stable 10.0.14 |
| Support F# de F* | **nouveau backend Custard** `FStarC.Custard.PrintFSharp` ; `fsharp/base`, `ulibfs` et `ulib/fs` **supprimés** (CHANGES.md « The F# library (`fsharp/base`, `ulibfs`) … are removed. The F# backend still exists ») | clone anonyme du tag : `fsharp/` ne contient plus que `README`, `Makefile`, `global.json` (SDK 10.0.100, rollForward latestMajor) et 2 tests |

## Tableau par étape
| # | Étape | Résultat | Preuve |
|---|---|---|---|
| 1 | Source de la passe 1 telle quelle | **KO** : `Error 134 … Namespace 'FStar.Mul' cannot be found` (FStar.Mul supprimé le 2026-04-16, commit `2b533ddc` ; `( * )` est maintenant dans `Prims`) | `10` |
| 2 | Vérification après retrait de `open FStar.Mul` (Z3 5.3.0) | **OK** en ~1,1 s, toutes les preuves passent sans indice supplémentaire | `11`, `11b` |
| 3 | Extraction `--codegen FSharp` de la source passe 1 (contournements compris) | **KO** : `Error 395 : Custard: FStar.UInt8.v has no F* definition and no F# realization` (`U8.v` dans `validate`/`parse`) | `20`, `21` |
| 3b | Idem avec `--custard_entry_module Core` après réécriture | **KO** : même Error 395, cette fois venant de la spec `to_nats` (la racine « module » extrait aussi le code de spec) | `22` |
| 3c | Source réécrite (`U8.v`/`U32.v` retirés du code calculatoire) + racines explicites `--custard_entry Core.<f>` ×14 | **OK** en 0,5 s : projet complet `Core.fs` (59 l.) + `FStarCustard.fs` (288 l., support embarqué) + `Core.fsproj` (net10.0) | `23` |
| 4 | `dotnet build -c Release` (JIT) | **OK, 0 warning, 0 erreur** (aucun `LangVersion`, aucun `--mlcompatibility`) | `30`, `31`, `70` |
| 5 | Tests JIT | **9201 vérifications, 0 échec** (mêmes tests qu'en passe 1, portés à la nouvelle API) | `40` |
| 6 | Mutations | F* : borne 64→65 (non équivalente) rejetée `Error 19` ; vecteur `hash [] = 0x811C9DC4` rejeté `Error 19` ; une mutation 64→65 sur `U8.lte n 64uy` seule est acceptée **à raison** (redondante : `n == length rest <= 64`). F# généré muté 64→65 : 33 échecs | `42`, `43` |
| 7 | `dotnet publish -r linux-x64 -p:PublishAot=true` | **OK en 10,9 s, 0 warning IL** (passe 1 : IL3053 + IL2104 sur FSharp.Core) | `50` |
| 8 | Tests AOT | **9201 / 0 échec** | `51`, `70` |
| 9 | Build propre complet `build.sh` (vérif + extraction + JIT + tests + AOT + tests) | **OK en 21,9 s** ; extraction identique octet à octet à `extracted/Core.fs` | `70` |

## Ce qui a changé par rapport à la passe 1
| Problème de la passe 1 | Passe 2 (F* 2026.10.25) |
|---|---|
| `#light "off"` refusé (FS0062) → `LangVersion 5.0` | **Résolu** : nouveau printer F# conscient de l'indentation ; en-tête `#nowarn "25" "26" "40" "49" "64" "1182" "3220"` ; aucun flag de langage |
| Littéraux `u8` extraits en `int` (`0xA5uy` → `0xA5`) | **Résolu** : `0xa5uy` ; j'ai remis les littéraux dans la source |
| Code de spec extrait (`to_nats`, `well_formed`…) → `noextract` | **Résolu avec racines explicites** (`--custard_entry`, seul le code atteignable est extrait, aucun `noextract`). **Toujours présent** avec `--custard_entry_module` (Error 395 sur `to_nats`) |
| `FStar_UInt8.t = bigint`, `bytes = bigint list` | **Résolu** : `uint8`, `uint32` natifs, `list<uint8>` ; conversions `byte[] ↔ bigint list` supprimées du harnais |
| `uint_to_t` via `UInt32.Parse(string x)` | **Résolu** : `FStar.Int.Cast.uint8_to_uint32` → `(uint32 b)` ; `U8.uint_to_t (length …)` → `(uint8 …)` |
| `option` F* ≠ `option` F# (adaptateur) | **Résolu** : option F# native ; record `msg {payload}` réduit à sa liste (§5.2 « newtype collapse ») |
| `ulibfs` non testé, 5 fichiers à la main (629 l.) | **Remplacé** : support généré et embarqué par projet (`FStarCustard.fs`) ; `fstar/fsharp-project/{ulibfs,core}` supprimés |
| FS0044 ×2, IL3053, IL2104 | **0 warning** FS et IL |
| `printf` incompatible AOT (harnais) | sans objet (harnais sans `printf` depuis la passe 1 ; le code généré n'en contient pas) |

**Nouveaux coûts** (erreurs exactes dans les logs) : (a) `FStar.Mul` supprimé (Error 134) ; (b) **`U8.v` / `U32.v` interdits dans le code extrait** (Error 395 : le backend F# n'accepte que ce que le backend C accepte) → `validate`/`parse` réécrits avec `len_ok` (comparaisons machine `U8.lte`, cast prouvé sans perte) et `checked_add` avec `U32.lte a (U32.sub 0xFFFFFFFFul b)` ; mêmes post-conditions, re-prouvées. Le message Error 395 ne nomme pas la définition fautive. (c) L'extraction exige un `.checked` préalable (`Error 317 … Module Core.fst was not checked`) : vérifier d'abord avec `--cache_checked_modules`. (d) Noms globaux préfixés (`Core.core_parse`) sans `--custard_split`.

## Lignes
| | lignes |
|---|---|
| `spec/Core.fst` | 179 (122 non vides hors commentaires) ; passe 1 : 177 |
| `extracted/Core.fs` | 59 (43 non vides) ; passe 1 : 106 (66) |
| Support généré `extracted/FStarCustard.fs` | 288 (passe 1 : 629 l. de `ulibfs` choisies à la main) |
| Harnais `fsharp-project/app/Program.fs` | 151 |

## Mesures (`logs/v2/60-measures.log`, 10 exécutions ; p95 = rang le plus proche, donc = max pour n=10 ; caches de pages vidés par `drop_caches` avant chaque run « froid »)
Machine partagée : loadavg ≈ 2,0 pendant la mesure (autres agents) ; chiffres indicatifs.
| | AOT passe 2 | AOT passe 1 | JIT passe 2 (`dotnet coreapp.dll`) | JIT passe 1 |
|---|---|---|---|---|
| Taille binaire | **2 352 784 o** (+4 062 000 o `.dbg` séparé) | 3 442 336 o | n/a | n/a |
| Démarrage à froid, médiane / p95 | **7,4 / 10,4 ms** | 11,2 / 19,8 ms | 83,6 / 175,1 ms | 100,4 / 115,5 ms |
| Démarrage caches chauds, médiane / p95 | 2,4 / 2,9 ms | non mesuré | 31,8 / 36,2 ms | non mesuré |
| RSS idle médiane | **4 314 kB** | 5 568 kB | 27 098 kB | 30 422 kB |
| Hash 64 o (2 M itérations, 2 passes) | **81–93 ns** | 12,7 µs | 85–91 ns | 7,6 µs |
`ldd` AOT : libm, libc, ld-linux seulement. Le hash est ≈ 140× plus rapide qu'en passe 1 (plus de `bigint` ni de `Parse(string)`) : ~1,3–1,5 ns/octet sur `list<uint8>`. Pas de baseline native mesurée ici, donc pas de ratio vs C/Rust.

## Limites
- Un module de premier ordre sur listes ; non testé : `Seq`, effets, `--custard_split`, tableaux. Le backend refuse explicitement (Error 395) ce qu'il ne sait pas réaliser plutôt que de générer du F# faux : c'est un progrès, mais l'écriture F* doit se limiter au sous-ensemble « extractible en C » (pas de `v` vers `nat` dans le code).
- Les preuves couvrent le code F*, pas `FStarCustard.fs` (généré, non vérifié) ni le harnais.

## Verdict : **utilisable, sans contournement de build**
Avec F* 2026.10.25, la chaîne F* → F# → NativeAOT passe de « utilisable avec contournements » à **propre** : projet généré complet, 0 warning FS/IL, aucun flag de langage, octets en `uint8`, binaire 2,35 Mo qui démarre en 7 ms et tient en 4,3 Mo de RSS, 9201 tests OK en JIT et en AOT. Le prix est côté source : `FStar.Mul` disparu et interdiction de `U8.v`/`U32.v` dans le code calculatoire (3 définitions réécrites, preuves inchangées), et il faut lister les racines d'extraction.
