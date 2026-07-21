import Sembla.IR

namespace Sembla.Plan

/-- Frozen composition artifact and identity strings from the composition PRD preamble. -/
def compositionSourceSchema : String := "sembla.composition-source/v1"
def planSchema : String := "sembla.executable-plan/v1"
def linkerSemantics : String := "sembla.linker/v1"
def stableIdentityScheme : String := "sembla.identity/stable-v1"
def legacyIdentityScheme : String := "sembla.identity/legacy-positional-v1"
def canonicalEncoding : String := "sembla.canonical-json/v1"
def sourceMapSchema : String := "sembla.source-map/v1"
def sourceArtifactDomain : String := "sembla.source-artifact/v1"
def planCoreDomain : String := "sembla.plan-core/v1"
def planEnvelopeDomain : String := "sembla.plan-envelope/v1"
def bundleRootDomain : String := "sembla.bundle-root/v1"
def ruleWordDomain : String := "sembla.rule-word/v1"
def hashAlgorithm : String := "sha256"
def globalSchedulerDomain : String := "domain:global"
def tauLeapAlgorithm : String := "tau_leap"

inductive PlanOrigin where
  | linked
  | directStable
deriving Repr, BEq

structure HashRecordV1 where
  algorithm : String
  domain : String
  digest : String
deriving Repr, BEq

structure LinkerDescriptorV1 where
  semantics : String
  sourceSchema : String
  planSchema : String
  identityScheme : String
  canonicalEncoding : String
  sourceMapSchema : String
deriving Repr, BEq

/-- PRD 0007 replaces this placeholder with the frozen source-map structure. -/
inductive SourceMapPlaceholder where
  | unavailable
deriving Repr, BEq

structure LinkedProvenanceV1 where
  sourceHash : HashRecordV1
  linker : LinkerDescriptorV1
  sourceMap : SourceMapPlaceholder
deriving Repr, BEq

structure SchedulerDomainV1 where
  id : String
  algorithm : String
  leaves : List String
deriving Repr, BEq

structure LeafIdentityV1 where
  box : String
  occurrence : String
deriving Repr, BEq

structure TransitionIdentityV1 where
  box : String
  name : String
  identity : String
  ruleWord : UInt32
deriving Repr, BEq

structure MailboxIdentityV1 where
  identity : String
  sourceBox : String
  sourcePort : String
  targetBox : String
  targetPort : String
deriving Repr, BEq

structure IdentityMapV1 where
  modelId : String
  enabledFeatures : List String
  schedulerDomains : List SchedulerDomainV1
  leaves : List LeafIdentityV1
  transitions : List TransitionIdentityV1
  mailboxes : List MailboxIdentityV1
deriving Repr, BEq

structure ExecutablePlanV1 where
  schemaVersion : String
  identityScheme : String
  origin : PlanOrigin
  model : IR.Model
  identity : IdentityMapV1
  linkedProvenance : Option LinkedProvenanceV1
deriving Repr, BEq

end Sembla.Plan
