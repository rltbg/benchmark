module Core_Low
(* Porte 2 : version Low* (buffers) du noyau validate / parse / hash, extractible en C par KaRaMeL.
   Trame : [0xA5][version=1][n][payload : n octets], n <= 64.
   Spec fonctionnelle sur Seq, mêmes définitions que /home/user/fstar-aot/spec/Core.fst (version liste). *)
open FStar.HyperStack.ST
open FStar.Mul
module B   = LowStar.Buffer
module HS  = FStar.HyperStack
module U8  = FStar.UInt8
module U32 = FStar.UInt32
module S   = FStar.Seq
module L   = C.Loops

type bytes = S.seq U8.t

inline_for_extraction noextract
let magic       : U8.t = 0xA5uy
inline_for_extraction noextract
let version     : U8.t = 1uy
noextract
let max_payload : n:nat{n < 256} = 64
noextract
let header_len  : nat = 3

(* ---------------- Spécification pure (Seq) ---------------- *)

noextract
let well_formed (s:bytes) : bool =
  S.length s >= header_len
  && S.index s 0 = magic && S.index s 1 = version
  && U8.v (S.index s 2) <= max_payload
  && S.length s = header_len + U8.v (S.index s 2)

noextract
type payload = p:bytes{S.length p <= max_payload}

noextract
let parse_spec (s:bytes) : option payload =
  if well_formed s then Some (S.slice s header_len (S.length s)) else None

noextract
let encode_spec (p:payload) : bytes =
  S.append (S.seq_of_list [magic; version; U8.uint_to_t (S.length p)]) p

(* FNV-1a 32 bits : h' = (h xor b) * prime  mod 2^32 *)
inline_for_extraction noextract
let fnv_offset : U32.t = 2166136261ul
inline_for_extraction noextract
let fnv_prime  : U32.t = 16777619ul

[@@ CInline]
let fnv_step (h:U32.t) (b:U8.t) : U32.t =
  U32.mul_mod (U32.logxor h (FStar.Int.Cast.uint8_to_uint32 b)) fnv_prime

(* hash_spec s = fold_left fnv_step offset s, écrit en snoc pour coller à la boucle *)
noextract
let rec hash_spec (s:bytes) : Tot U32.t (decreases S.length s) =
  if S.length s = 0 then fnv_offset
  else fnv_step (hash_spec (S.slice s 0 (S.length s - 1))) (S.index s (S.length s - 1))

(* Spec mathématique sur entiers non bornés (aucune hypothèse machine) *)
noextract
let fnv_step_nat (h:nat{h < pow2 32}) (b:nat{b < 256}) : (r:nat{r < pow2 32}) =
  (FStar.UInt.logxor #32 h b * 16777619) % pow2 32

noextract
let rec hash_nat (s:bytes) : Tot (r:nat{r < pow2 32}) (decreases S.length s) =
  if S.length s = 0 then 2166136261
  else fnv_step_nat (hash_nat (S.slice s 0 (S.length s - 1))) (U8.v (S.index s (S.length s - 1)))

(* ---------------- Lemmes purs ---------------- *)

(* Borne de longueur : tout payload accepté a <= 64 octets, trame <= 67 *)
let parse_length_bound (s:bytes)
  : Lemma (match parse_spec s with
           | None -> True
           | Some p -> S.length p <= max_payload /\ S.length s == header_len + S.length p
                     /\ S.length s <= header_len + max_payload)
  = ()

let parse_rejects_short (s:bytes)
  : Lemma (requires S.length s < header_len) (ensures None? (parse_spec s)) = ()

let roundtrip (p:payload)
  : Lemma (parse_spec (encode_spec p) == Some p)
  = let e = encode_spec p in
    assert (S.length e == header_len + S.length p);
    assert (S.index e 0 == magic /\ S.index e 1 == version);
    assert (S.index e 2 == U8.uint_to_t (S.length p));
    assert (S.equal (S.slice e header_len (S.length e)) p)

let fnv_step_correct (h:U32.t) (b:U8.t)
  : Lemma (U32.v (fnv_step h b) == fnv_step_nat (U32.v h) (U8.v b)) = ()

(* Le hash machine (mul_mod) coïncide avec la spec sur entiers non bornés *)
let rec hash_correct (s:bytes)
  : Lemma (ensures U32.v (hash_spec s) == hash_nat s) (decreases S.length s)
  = if S.length s = 0 then ()
    else begin
      let s' = S.slice s 0 (S.length s - 1) in
      hash_correct s';
      fnv_step_correct (hash_spec s') (S.index s (S.length s - 1))
    end

(* Vecteurs connus *)
let hash_empty () : Lemma (hash_spec S.empty == 0x811C9DC5ul) = ()
let hash_snoc (s:bytes) (x:U8.t)
  : Lemma (hash_spec (S.snoc s x) == fnv_step (hash_spec s) x)
  = assert (S.equal (S.slice (S.snoc s x) 0 (S.length s)) s)

noextract
let sn (s:bytes) (x:U8.t) : bytes = S.snoc s x

let hash_a () : Lemma (hash_spec (sn S.empty 97uy) == 0xE40C292Cul) =
  hash_snoc S.empty 97uy;
  assert_norm (fnv_step fnv_offset 97uy == 0xE40C292Cul)

let hash_foobar ()
  : Lemma (hash_spec (sn (sn (sn (sn (sn (sn S.empty 102uy) 111uy) 111uy) 98uy) 97uy) 114uy) == 0xBF9CF968ul) =
  let s0 = S.empty in
  let s1 = sn s0 102uy in let s2 = sn s1 111uy in let s3 = sn s2 111uy in
  let s4 = sn s3 98uy in let s5 = sn s4 97uy in
  hash_snoc s0 102uy; hash_snoc s1 111uy; hash_snoc s2 111uy;
  hash_snoc s3 98uy; hash_snoc s4 97uy; hash_snoc s5 114uy;
  assert_norm (fnv_step (fnv_step (fnv_step (fnv_step (fnv_step (fnv_step fnv_offset 102uy) 111uy) 111uy) 98uy) 97uy) 114uy == 0xBF9CF968ul)

(* ---------------- Fonctions Low* ---------------- *)

/// Vue de l'entrée : les [len] premiers octets du buffer.
let view (h:HS.mem) (b:B.buffer U8.t) (len:U32.t{U32.v len <= B.length b}) : GTot bytes =
  S.slice (B.as_seq h b) 0 (U32.v len)

/// validate : lit au plus 3 octets (indices < len garantis), jamais hors bornes.
val validate (b:B.buffer U8.t) (len:U32.t)
  : Stack bool
    (requires fun h -> B.live h b /\ U32.v len <= B.length b)
    (ensures  fun h0 r h1 -> h0 == h1 /\ (r <==> well_formed (view h0 b len)))
let validate b len =
  if U32.lt len 3ul then false
  else begin
    let m = B.index b 0ul in
    let v = B.index b 1ul in
    let n = B.index b 2ul in
    // n < 256 donc 3 + n ne déborde pas ; preuve par le typeur (U32.add exige la précondition)
    U8.eq m magic && U8.eq v version && U8.lte n 64uy
    && U32.eq len (U32.add 3ul (FStar.Int.Cast.uint8_to_uint32 n))
  end

/// parse : copie le payload dans [out] (capacité [cap]) et écrit sa longueur dans out_len[0].
/// Retourne true ssi la trame est bien formée ET le payload tient dans cap.
val parse (b:B.buffer U8.t) (len:U32.t) (out:B.buffer U8.t) (cap:U32.t) (out_len:B.buffer U32.t)
  : Stack bool
    (requires fun h ->
      B.live h b /\ B.live h out /\ B.live h out_len /\
      U32.v len <= B.length b /\ B.length out == U32.v cap /\ B.length out_len == 1 /\
      B.disjoint b out /\ B.disjoint b out_len /\ B.disjoint out out_len)
    (ensures fun h0 r h1 ->
      B.modifies (B.loc_union (B.loc_buffer out) (B.loc_buffer out_len)) h0 h1 /\
      (r <==> (Some? (parse_spec (view h0 b len)) /\
               S.length (Some?.v (parse_spec (view h0 b len))) <= U32.v cap)) /\
      (r ==> (let p = Some?.v (parse_spec (view h0 b len)) in
              let n = U32.v (B.get h1 out_len 0) in
              n == S.length p /\ n <= max_payload /\ n <= U32.v cap /\
              S.equal (S.slice (B.as_seq h1 out) 0 n) p)))
let parse b len out cap out_len =
  if not (validate b len) then false
  else begin
    let n = U32.sub len 3ul in            // len >= 3 car validate
    if U32.gt n cap then false
    else begin
      B.blit b 3ul out 0ul n;             // memcpy ; disjoint + bornes exigés par la précondition de blit
      B.upd out_len 0ul n;
      true
    end
  end

/// hash FNV-1a 32 bits des [len] premiers octets ; total, sans débordement (mul_mod).
val hash (b:B.buffer U8.t) (len:U32.t)
  : Stack U32.t
    (requires fun h -> B.live h b /\ U32.v len <= B.length b)
    (ensures  fun h0 r h1 -> B.modifies B.loc_none h0 h1 /\ r == hash_spec (view h0 b len) /\
                             U32.v r == hash_nat (view h0 b len))
let hash b len =
  let h0 = get () in
  push_frame ();
  let acc = B.alloca fnv_offset 1ul in
  let h1 = get () in
  let inv (h:HS.mem) (i:nat) : GTot Type0 =
    B.live h b /\ B.live h acc /\ B.modifies (B.loc_buffer acc) h1 h /\
    B.as_seq h b == B.as_seq h1 b /\
    i <= U32.v len /\
    B.get h acc 0 == hash_spec (S.slice (B.as_seq h1 b) 0 i) in
  let body (i:U32.t{0 <= U32.v i /\ U32.v i < U32.v len})
    : Stack unit (requires fun h -> inv h (U32.v i))
                 (ensures fun hA _ hB -> inv hA (U32.v i) /\ inv hB (U32.v i + 1))
    = let x = B.index b i in
      let a = B.index acc 0ul in
      B.upd acc 0ul (fnv_step a x);
      let hB = get () in
      let s = B.as_seq h1 b in
      assert (S.equal (S.slice (S.slice s 0 (U32.v i + 1)) 0 (U32.v i)) (S.slice s 0 (U32.v i)))
  in
  L.for 0ul len inv body;
  let r = B.index acc 0ul in
  hash_correct (view h0 b len);
  let h2 = get () in
  assert (S.equal (S.slice (B.as_seq h1 b) 0 (U32.v len)) (view h0 b len));
  pop_frame ();
  r
