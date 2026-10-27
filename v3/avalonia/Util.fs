module Util
// Utilitaires communs : fonctions pures de la spec (fnv, xorshift32, hauteurs, couleurs), arguments, JSON manuel.
// Aucun printf/sprintf F# (reflexion, IL2026 en NativeAOT) : concatenation de chaines uniquement.

open System
open System.Globalization
open System.IO
open System.Text

let inv = CultureInfo.InvariantCulture

// ---------------------------------------------------------------- fonctions pures (spec v3 section 4)
/// FNV-1a 32 bits des 4 octets little-endian de i
let inline fnv (i: int) : uint32 =
    let u = uint32 i
    let mutable h = 2166136261u
    h <- (h ^^^ (u &&& 0xFFu)) * 16777619u
    h <- (h ^^^ ((u >>> 8) &&& 0xFFu)) * 16777619u
    h <- (h ^^^ ((u >>> 16) &&& 0xFFu)) * 16777619u
    h <- (h ^^^ ((u >>> 24) &&& 0xFFu)) * 16777619u
    h

/// hauteur standard h(i) = 48 + 16*(fnv(i) % 6)
let inline hStd (i: int) = 48 + 16 * int (fnv i % 6u)
/// hauteur dense hd(i) = 24 + 4*(fnv(i) % 6)
let inline hDense (i: int) = 24 + 4 * int (fnv i % 6u)
let inline rgbOf (i: int) =
    let h = fnv i
    (byte (h &&& 255u), byte ((h >>> 8) &&& 255u), byte ((h >>> 16) &&& 255u))
let inline initialPrice (i: int) = float (fnv (i + 7) % 100000u) / 100.0
let inline xorshift32 (x: uint32) =
    let mutable x = x
    x <- x ^^^ (x <<< 13)
    x <- x ^^^ (x >>> 17)
    x <- x ^^^ (x <<< 5)
    x
let SEED = 0x2545F491u

// ---------------------------------------------------------------- racine du depot
let benchRoot =
    lazy (
        let env = Environment.GetEnvironmentVariable "BENCH_ROOT"
        if not (String.IsNullOrEmpty env) then env
        else
            let mutable d = DirectoryInfo(AppContext.BaseDirectory)
            let mutable res = null
            while isNull res && not (isNull d) do
                if File.Exists(Path.Combine(d.FullName, "core", "benchcore.h")) then res <- d.FullName
                d <- d.Parent
            if isNull res then Directory.GetCurrentDirectory() else res)

// ---------------------------------------------------------------- arguments
type Args =
    { mutable Scenario: string
      mutable Duration: float
      mutable Out: string
      mutable N: int64
      mutable NGiven: bool
      mutable Assets: string
      Variants: Collections.Generic.Dictionary<string, string> }
    member a.Variant(k: string, dflt: string) =
        match a.Variants.TryGetValue k with
        | true, v -> v
        | _ -> dflt
    member a.Has(k: string) = a.Variants.ContainsKey k

let parseArgs (argv: string[]) =
    let a = { Scenario = "idle"; Duration = 12.0; Out = "out.json"; N = 0L; NGiven = false; Assets = null
              Variants = Collections.Generic.Dictionary<string, string>() }
    let mutable i = 0
    while i < argv.Length do
        let nxt () = i <- i + 1; argv.[i]
        match argv.[i] with
        | "--scenario" -> a.Scenario <- nxt ()
        | "--duration" -> a.Duration <- Double.Parse(nxt (), inv)
        | "--out" -> a.Out <- nxt ()
        | "--n" | "--rate" | "--shapes" -> a.N <- Int64.Parse(nxt (), inv); a.NGiven <- true
        | "--assets" -> a.Assets <- nxt ()
        | "--variant" ->
            let v = nxt ()
            let k = v.IndexOf '='
            // k=v ; une valeur nue "x" est lue comme feature=x
            if k > 0 then a.Variants.[v.Substring(0, k)] <- v.Substring(k + 1) else a.Variants.["feature"] <- v
        | _ -> ()
        i <- i + 1
    if isNull a.Assets then a.Assets <- Path.Combine(benchRoot.Value, "assets")
    a

// ---------------------------------------------------------------- JSON manuel
let kvI (sb: StringBuilder) (k: string) (v: int64) = sb.Append('"').Append(k).Append("\":").Append(v.ToString(inv)).Append(',') |> ignore
let kvU (sb: StringBuilder) (k: string) (v: uint64) = sb.Append('"').Append(k).Append("\":").Append(v.ToString(inv)).Append(',') |> ignore
let kvF (sb: StringBuilder) (k: string) (v: float) =
    let s = if Double.IsNaN v || Double.IsInfinity v then "null" else v.ToString("F3", inv)
    sb.Append('"').Append(k).Append("\":").Append(s).Append(',') |> ignore
let kvS (sb: StringBuilder) (k: string) (v: string) =
    sb.Append('"').Append(k).Append("\":\"").Append(v.Replace("\\", "/").Replace("\"", "'")).Append("\",") |> ignore
let kvRaw (sb: StringBuilder) (k: string) (v: string) = sb.Append('"').Append(k).Append("\":").Append(v).Append(',') |> ignore
let kvFArr (sb: StringBuilder) (k: string) (v: seq<float>) =
    sb.Append('"').Append(k).Append("\":[") |> ignore
    let mutable first = true
    for x in v do
        if not first then sb.Append(',') |> ignore
        first <- false
        sb.Append(x.ToString("F3", inv)) |> ignore
    sb.Append("],") |> ignore

let clamp (lo: float) (hi: float) (v: float) = max lo (min hi v)
let ticksToMs (t: int64) = float t * 1000.0 / float Diagnostics.Stopwatch.Frequency

/// Construit un mot court deterministe de 8 caracteres a partir de fnv (T2)
let fnvText8 (v: uint32) (dst: Span<char>) =
    let a = "0123456789abcdefghijklmnopqrstuvwxyz"
    let mutable h = v
    for k in 0 .. 7 do
        h <- xorshift32 (h + 0x9E3779B9u)
        dst.[k] <- a.[int (h % 36u)]

/// pseudo-aleatoire deterministe [0,1)
let inline rnd01 (i: int) = float (fnv i &&& 0xFFFFFFu) / 16777216.0
