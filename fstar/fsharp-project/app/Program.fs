module Program
// Tests qui reprennent, lemme par lemme, la spec de spec/Core.fst.
// Mode par défaut : suite de tests (code retour 0 = tout passe).
//   coreapp            -> tests
//   coreapp once       -> parse+hash d'une trame, puis exit (mesure du démarrage)
//   coreapp bench N    -> N hash de 64 octets (micro-benchmark)
open System
open System.Numerics

// Adaptateur : l'option d'F* (FStar_Pervasives_Native.option) n'est pas l'option d'F#.
let o (x: 'a FStar_Pervasives_Native.option) : 'a option =
    match x with FStar_Pervasives_Native.Some v -> Some v | FStar_Pervasives_Native.None -> None
let parse l = o (Core.parse l)
let hashFrame l = o (Core.hash_frame l)
let checkedAdd a b = o (Core.checked_add a b)
let toL (a: byte[]) : bigint list = [ for b in a -> BigInteger(int b) ]
let ofL (l: bigint list) : byte[] = l |> List.map byte |> Array.ofList
let frame (payload: byte[]) = Array.append [| 0xA5uy; 1uy; byte payload.Length |] payload

// Référence indépendante de la spec : FNV-1a 32 en uint64 natif (pas de bigint, pas de F*).
let fnvRef (a: byte[]) =
    let mutable h = 2166136261UL
    for b in a do h <- ((h ^^^ uint64 b) * 16777619UL) &&& 0xFFFFFFFFUL
    uint32 h
let hashOf (a: byte[]) : uint32 = Core.hash (toL a)

let mutable passed = 0
let failures = Collections.Generic.List<string>()
let check name (cond: bool) =
    if cond then passed <- passed + 1 else failures.Add name

// PRNG déterministe (xorshift) pour les propriétés
let mutable st = 0x2545F491u
let rnd () =
    st <- st ^^^ (st <<< 13); st <- st ^^^ (st >>> 17); st <- st ^^^ (st <<< 5); st
let rndBytes n = Array.init n (fun _ -> byte (rnd () &&& 0xFFu))

let runTests () =
    // --- vecteurs FNV-1a 32 officiels ---
    check "hash_empty (Lemme hash_empty)" (hashOf [||] = 0x811C9DC5u)
    check "hash_a (Lemme hash_a)" (hashOf [| 97uy |] = 0xE40C292Cu)
    check "hash_foobar" (hashOf (Text.Encoding.ASCII.GetBytes "foobar") = 0xBF9CF968u)
    check "hash_foo" (hashOf (Text.Encoding.ASCII.GetBytes "foo") = 0xA9F37ED7u)
    // --- hash = référence indépendante (Lemme hash_correct) ---
    for n in [0;1;2;3;7;63;64;65;255;1000] do
        let a = rndBytes n
        check ("hash_correct len=" + string n) (hashOf a = fnvRef a)
    for _ in 1 .. 300 do
        let a = rndBytes (int (rnd () % 300u))
        check "hash_correct random" (hashOf a = fnvRef a)
    // octets extrêmes (xor puis mul_mod sur 0x00 / 0xFF)
    check "hash_all_ff" (hashOf (Array.create 100 0xFFuy) = fnvRef (Array.create 100 0xFFuy))
    check "hash_all_00" (hashOf (Array.zeroCreate 100) = fnvRef (Array.zeroCreate 100))
    // --- Lemme fold_append : hash incrémental ---
    for _ in 1 .. 200 do
        let a = rndBytes (int (rnd () % 50u)) in let b = rndBytes (int (rnd () % 50u))
        let direct = Core.fnv_fold Core.fnv_offset (toL (Array.append a b))
        let incr = Core.fnv_fold (Core.fnv_fold Core.fnv_offset (toL a)) (toL b)
        check "fold_append" (direct = incr)
    // --- validate / parse : cas limites ---
    check "empty rejected" (not (Core.validate []) && (parse []).IsNone)
    check "parse_rejects_short len0..2" (
        [ [||]; [|0xA5uy|]; [|0xA5uy;1uy|] ] |> List.forall (fun a -> (parse (toL a)).IsNone && not (Core.validate (toL a))))
    check "empty payload ok" (Core.validate (toL (frame [||])) && (parse (toL (frame [||]))).Value.payload.IsEmpty)
    check "payload 64 ok (max_payload)" (Core.validate (toL (frame (rndBytes 64))))
    check "payload 65 rejected (len byte=65)" (not (Core.validate (toL (frame (rndBytes 65)))))
    check "payload 255 rejected" (not (Core.validate (toL (frame (rndBytes 255)))))
    let bad_magic = frame (rndBytes 4) in bad_magic.[0] <- 0xA4uy
    check "bad magic" (not (Core.validate (toL bad_magic)))
    let bad_ver = frame (rndBytes 4) in bad_ver.[1] <- 2uy
    check "bad version" (not (Core.validate (toL bad_ver)))
    check "version 0 rejected" (let f = frame [||] in f.[1] <- 0uy; not (Core.validate (toL f)))
    // Lemme parse_rejects_bad_len : n déclaré <> octets réels
    let f = frame (rndBytes 10)
    check "bad_len too few" (parse (toL f.[0..f.Length-2])).IsNone
    check "bad_len too many" (parse (toL (Array.append f [|0uy|]))).IsNone
    check "bad_len declared 0 with data" (let g = Array.copy f in g.[2] <- 0uy; (parse (toL g)).IsNone)
    check "bad_len declared 255 (byte max)" (let g = Array.copy f in g.[2] <- 255uy; (parse (toL g)).IsNone)
    // --- Lemme parse_validate_iff + parse_length_bound sur entrées aléatoires ---
    let mutable accepted = 0
    for i in 1 .. 4000 do
        let a =
            if i % 2 = 0 then rndBytes (int (rnd () % 80u))
            else (let p = rndBytes (int (rnd () % 70u)) in let f = frame p in (if rnd () % 4u = 0u then f.[int (rnd () % 3u)] <- byte (rnd ())); f)
        let l = toL a
        let p = parse l
        check "parse_validate_iff" ((p.IsSome) = Core.validate l)
        match p with
        | Some m ->
            accepted <- accepted + 1
            let n = m.payload.Length
            check "parse_length_bound: payload<=64" (n <= 64)
            check "parse_length_bound: total=3+n<=67" (a.Length = 3 + n && a.Length <= 67)
            check "payload = drop 3" (ofL m.payload = a.[3..])
        | None -> ()
    check "assez de trames acceptées pour être significatif" (accepted > 200)
    // --- Lemme roundtrip + encode_length ---
    for n in [0;1;2;32;63;64] do
        let p = rndBytes n
        let m : Core.msg = { payload = toL p }
        let e = Core.encode m
        check ("encode_length n=" + string n) (e.Length = 3 + n && e.Length <= 67)
        check ("roundtrip validate n=" + string n) (Core.validate e)
        check ("roundtrip parse n=" + string n) ((parse e) = Some m)
        check ("encode bytes n=" + string n) (ofL e = frame p)
    // --- hash_frame ---
    let p = Text.Encoding.ASCII.GetBytes "hello"
    check "hash_frame ok" (hashFrame (toL (frame p)) = Some (fnvRef p))
    check "hash_frame none" (hashFrame (toL (frame p).[1..]) = None)
    // --- checked_add : bornes 32 bits (absence de débordement) ---
    let M = UInt32.MaxValue
    check "checked_add 0+0" (checkedAdd 0u 0u = Some 0u)
    check "checked_add max+0" (checkedAdd M 0u = Some M)
    check "checked_add max-1+1" (checkedAdd (M-1u) 1u = Some M)
    check "checked_add max+1 = None" (checkedAdd M 1u = None)
    check "checked_add max+max = None" (checkedAdd M M = None)
    check "checked_add 2^31+2^31 = None" (checkedAdd 0x80000000u 0x80000000u = None)
    check "checked_add 2^31+(2^31-1) = Some" (checkedAdd 0x80000000u 0x7FFFFFFFu = Some M)
    for _ in 1 .. 500 do
        let a = rnd () in let b = rnd ()
        let exp = let s = uint64 a + uint64 b in if s < 4294967296UL then Some (uint32 s) else None
        check "checked_add random" (checkedAdd a b = exp)
    // --- constantes ---
    check "constants" (Core.magic = BigInteger 165 && Core.version = BigInteger.One && Core.max_payload = BigInteger 64 && Core.header_len = BigInteger 3)
    Console.WriteLine("TESTS: " + string passed + " passed, " + string failures.Count + " failed")
    for f in failures |> Seq.truncate 20 do Console.WriteLine("  FAIL: " + f)
    if failures.Count = 0 then 0 else 1

[<EntryPoint>]
let main argv =
    match argv with
    | [| "once" |] ->
        let f = toL (frame (Text.Encoding.ASCII.GetBytes "hello"))
        match hashFrame f with
        | Some h -> Console.WriteLine(h.ToString("X8")); 0
        | None -> 1
    | [| "idle" |] ->
        let f = toL (frame (Text.Encoding.ASCII.GetBytes "hello"))
        let r = hashFrame f
        Console.WriteLine("ready"); Console.Out.Flush()
        Threading.Thread.Sleep 4000
        (if r.IsSome then 0 else 1)
    | [| "bench"; n |] ->
        let l = toL (rndBytes 64)
        let sw = Diagnostics.Stopwatch.StartNew()
        let mutable acc = 0u
        for _ in 1 .. int n do acc <- acc ^^^ Core.hash l
        Console.WriteLine("bench: " + n + " hash(64B) in " + string sw.ElapsedMilliseconds + " ms (acc=" + acc.ToString("X8") + ") => " + string (int (float sw.ElapsedTicks * 1e9 / float Diagnostics.Stopwatch.Frequency / float (int n))) + " ns/hash")
        0
    | _ -> runTests ()
