/-
  Monster Group SELinux Policy — merged static + dynamic
  196,883 = 71 × 59 × 47 access cells
-/

-- Monster dimensions
def SHARDS : Nat := 71
def SECTORS : Nat := 59
def ZONES : Nat := 47
def MONSTER_DIM : Nat := SHARDS * SECTORS * ZONES  -- 196883

-- Service types (mapped to shards mod 71)
inductive SvcType where
  | activity_profile
  | agent_docker
  | eco_research
  | fractran_meta_compiler
  | fractranllama
  | harbor_shard58
  | kagenti_portal
  | kiro1_shard42
  | kiro_feed_uucp
  | kiro_qms
  | kiro_session
  | moltis
  | monster_zone_0
  | monster_zone_1
  | monster_zone_2
  | monster_zone_3
  | monster_zone_4
  | monster_zone_5
  | monster_zone_6
  | monster_zone_7
  | monster_zone_8
  | monster_zone_9
  | nixwars_frens
  | openclaw
  | pastebin
  | rust_compiler
  | rust_mcp_services
  | snm_paste_server
  | solana
  | solfunmeme_service
  | unified_p2p_server
  | uucp_solana_gateway
  | wg_stego_tunnel
  | zkperf_da51
  | zone42_understanding
  | zos_server
  deriving DecidableEq, Repr

def toShard : SvcType → Fin SHARDS
  | .activity_profile => ⟨31, by omega⟩
  | .agent_docker => ⟨27, by omega⟩
  | .eco_research => ⟨37, by omega⟩
  | .fractran_meta_compiler => ⟨58, by omega⟩
  | .fractranllama => ⟨50, by omega⟩
  | .harbor_shard58 => ⟨2, by omega⟩
  | .kagenti_portal => ⟨59, by omega⟩
  | .kiro1_shard42 => ⟨31, by omega⟩
  | .kiro_feed_uucp => ⟨66, by omega⟩
  | .kiro_qms => ⟨61, by omega⟩
  | .kiro_session => ⟨15, by omega⟩
  | .moltis => ⟨65, by omega⟩
  | .monster_zone_0 => ⟨58, by omega⟩
  | .monster_zone_1 => ⟨26, by omega⟩
  | .monster_zone_2 => ⟨44, by omega⟩
  | .monster_zone_3 => ⟨37, by omega⟩
  | .monster_zone_4 => ⟨35, by omega⟩
  | .monster_zone_5 => ⟨69, by omega⟩
  | .monster_zone_6 => ⟨35, by omega⟩
  | .monster_zone_7 => ⟨41, by omega⟩
  | .monster_zone_8 => ⟨22, by omega⟩
  | .monster_zone_9 => ⟨5, by omega⟩
  | .nixwars_frens => ⟨25, by omega⟩
  | .openclaw => ⟨61, by omega⟩
  | .pastebin => ⟨38, by omega⟩
  | .rust_compiler => ⟨15, by omega⟩
  | .rust_mcp_services => ⟨28, by omega⟩
  | .snm_paste_server => ⟨34, by omega⟩
  | .solana => ⟨51, by omega⟩
  | .solfunmeme_service => ⟨58, by omega⟩
  | .unified_p2p_server => ⟨17, by omega⟩
  | .uucp_solana_gateway => ⟨36, by omega⟩
  | .wg_stego_tunnel => ⟨5, by omega⟩
  | .zkperf_da51 => ⟨26, by omega⟩
  | .zone42_understanding => ⟨63, by omega⟩
  | .zos_server => ⟨7, by omega⟩

inductive Perm where
  | read | write | execute | connect | setuid
  deriving DecidableEq, Repr

def toSector : Perm → Fin SECTORS
  | .read => ⟨0, by omega⟩
  | .write => ⟨1, by omega⟩
  | .execute => ⟨5, by omega⟩
  | .connect => ⟨8, by omega⟩
  | .setuid => ⟨14, by omega⟩

def monsterCell (shard : Fin SHARDS) (sector : Fin SECTORS) (zone : Fin ZONES) : Fin MONSTER_DIM :=
  ⟨shard.val * SECTORS * ZONES + sector.val * ZONES + zone.val,
   by omega⟩

-- Observed access cells: 168 of 196883
-- Coverage: 0.0853%

def allowed (svc : SvcType) (p : Perm) : Bool :=
  match svc, p with
  | .activity_profile, .connect => true
  | .activity_profile, .read => true
  | .activity_profile, .write => true
  | .agent_docker, .read => true
  | .eco_research, .connect => true
  | .eco_research, .read => true
  | .eco_research, .write => true
  | .fractran_meta_compiler, .connect => true
  | .fractran_meta_compiler, .read => true
  | .fractran_meta_compiler, .write => true
  | .fractranllama, .connect => true
  | .fractranllama, .read => true
  | .fractranllama, .write => true
  | .harbor_shard58, .connect => true
  | .harbor_shard58, .read => true
  | .harbor_shard58, .write => true
  | .kagenti_portal, .connect => true
  | .kagenti_portal, .read => true
  | .kagenti_portal, .write => true
  | .kiro1_shard42, .connect => true
  | .kiro1_shard42, .read => true
  | .kiro1_shard42, .write => true
  | .kiro_feed_uucp, .connect => true
  | .kiro_feed_uucp, .read => true
  | .kiro_feed_uucp, .write => true
  | .kiro_qms, .connect => true
  | .kiro_qms, .read => true
  | .kiro_qms, .write => true
  | .kiro_session, .connect => true
  | .kiro_session, .read => true
  | .kiro_session, .write => true
  | .moltis, .connect => true
  | .moltis, .read => true
  | .moltis, .write => true
  | .monster_zone_0, .connect => true
  | .monster_zone_0, .read => true
  | .monster_zone_0, .write => true
  | .monster_zone_1, .connect => true
  | .monster_zone_1, .read => true
  | .monster_zone_1, .write => true
  | .monster_zone_2, .connect => true
  | .monster_zone_2, .read => true
  | .monster_zone_2, .write => true
  | .monster_zone_3, .connect => true
  | .monster_zone_3, .read => true
  | .monster_zone_3, .write => true
  | .monster_zone_4, .connect => true
  | .monster_zone_4, .read => true
  | .monster_zone_4, .write => true
  | .monster_zone_5, .connect => true
  | .monster_zone_5, .read => true
  | .monster_zone_5, .write => true
  | .monster_zone_6, .connect => true
  | .monster_zone_6, .read => true
  | .monster_zone_6, .write => true
  | .monster_zone_7, .connect => true
  | .monster_zone_7, .read => true
  | .monster_zone_7, .write => true
  | .monster_zone_8, .connect => true
  | .monster_zone_8, .read => true
  | .monster_zone_8, .write => true
  | .monster_zone_9, .connect => true
  | .monster_zone_9, .read => true
  | .monster_zone_9, .write => true
  | .nixwars_frens, .connect => true
  | .nixwars_frens, .read => true
  | .nixwars_frens, .write => true
  | .openclaw, .connect => true
  | .openclaw, .read => true
  | .openclaw, .write => true
  | .pastebin, .connect => true
  | .pastebin, .read => true
  | .pastebin, .write => true
  | .rust_compiler, .read => true
  | .rust_mcp_services, .connect => true
  | .rust_mcp_services, .read => true
  | .rust_mcp_services, .write => true
  | .snm_paste_server, .connect => true
  | .snm_paste_server, .read => true
  | .snm_paste_server, .write => true
  | .solana, .connect => true
  | .solana, .read => true
  | .solana, .write => true
  | .solfunmeme_service, .connect => true
  | .solfunmeme_service, .read => true
  | .solfunmeme_service, .write => true
  | .unified_p2p_server, .connect => true
  | .unified_p2p_server, .read => true
  | .unified_p2p_server, .write => true
  | .uucp_solana_gateway, .connect => true
  | .uucp_solana_gateway, .read => true
  | .uucp_solana_gateway, .write => true
  | .wg_stego_tunnel, .connect => true
  | .wg_stego_tunnel, .read => true
  | .wg_stego_tunnel, .write => true
  | .zkperf_da51, .connect => true
  | .zkperf_da51, .read => true
  | .zkperf_da51, .write => true
  | .zone42_understanding, .connect => true
  | .zone42_understanding, .read => true
  | .zone42_understanding, .write => true
  | .zos_server, .connect => true
  | .zos_server, .read => true
  | .zos_server, .write => true
  | _, _ => false

def denied (svc : SvcType) (p : Perm) : Bool :=
  match svc, p with
  | .fractran_meta_compiler, .setuid => true
  | .harbor_shard58, .setuid => true
  | .zkperf_da51, .setuid => true
  | .unified_p2p_server, .setuid => true
  | .monster_zone_0, .setuid => true
  | .monster_zone_1, .setuid => true
  | .monster_zone_2, .setuid => true
  | .monster_zone_3, .setuid => true
  | .monster_zone_4, .setuid => true
  | .monster_zone_5, .setuid => true
  | .monster_zone_6, .setuid => true
  | .monster_zone_7, .setuid => true
  | .monster_zone_8, .setuid => true
  | .monster_zone_9, .setuid => true
  | .fractranllama, .setuid => true
  | .kagenti_portal, .setuid => true
  | .kiro1_shard42, .setuid => true
  | .kiro_feed_uucp, .setuid => true
  | .kiro_qms, .setuid => true
  | .kiro_session, .setuid => true
  | .moltis, .setuid => true
  | .nixwars_frens, .setuid => true
  | .openclaw, .setuid => true
  | .pastebin, .setuid => true
  | .rust_mcp_services, .setuid => true
  | .solfunmeme_service, .setuid => true
  | .wg_stego_tunnel, .setuid => true
  | .zos_server, .setuid => true
  | _, _ => false

-- Soundness: no permission is both allowed and denied
theorem policy_consistent (svc : SvcType) (p : Perm) :
    denied svc p = true → allowed svc p = false := by
  intro h; cases svc <;> cases p <;> simp_all [denied, allowed]

-- Monster torus: each (shard, sector, zone) maps to unique cell
theorem cell_injective (s1 s2 : Fin SHARDS) (p1 p2 : Fin SECTORS) (z1 z2 : Fin ZONES) :
    monsterCell s1 p1 z1 = monsterCell s2 p2 z2 →
    s1 = s2 ∧ p1 = p2 ∧ z1 = z2 := by
  intro h; simp [monsterCell, Fin.ext_iff] at h; omega
