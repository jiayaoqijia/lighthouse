# FOCIL (EIP-7805) 实现代码审查报告

> 审查日期: 2026-03-18
> 审查范围: Lighthouse CL FOCIL 实现 (7个提交)

---

## 审查总结

| 类别 | 数量 |
|------|------|
| 🔴 P0 阻塞问题 | 3 |
| 🟡 P1 重要问题 | 4 |
| 🟢 P2 建议改进 | 3 |

---

## P0 阻塞问题

### 1. ForkName 枚举缺少 `Heze`

**文件**: `consensus/types/src/fork/fork_name.rs`

**问题**: 规范定义 Heze fork 包含 EIP-7805，但 ForkName 枚举只到 Gloas，缺少 Heze。

**规范要求**:
```python
# specs/heze/fork-choice.md
if epoch >= HEZE_FORK_EPOCH:
    return HEZE_FORK_VERSION
```

**当前实现**:
```rust
pub enum ForkName {
    Base,
    Altair,
    Bellatrix,
    Capella,
    Deneb,
    Electra,
    Fulu,
    Gloas,
    // 缺少 Heze
}
```

**影响**: 无法激活 FOCIL 功能，所有 Heze-specific 逻辑无法执行。

**修复方案**: 在 `ForkName` 枚举中添加 `Heze`，并更新所有相关方法。

---

### 2. InclusionListStore 存储类型错误

**文件**: `beacon_node/beacon_chain/src/inclusion_list_verification.rs`

**问题**: 存储的类型与规范不一致。

**规范定义**:
```python
# specs/heze/inclusion-list.md
@dataclass
class InclusionListStore(object):
    inclusion_lists: DefaultDict[Tuple[Slot, Root], Set[InclusionList]]
    equivocators: DefaultDict[Tuple[Slot, Root], Set[ValidatorIndex]]
```

**当前实现**:
```rust
pub struct InclusionListStore<E: EthSpec> {
    inclusion_lists: RwLock<HashMap<StoreKey, HashSet<SignedInclusionList<E>>>>,
    // ...
}
```

**差异**:
- 规范存储 `Set[InclusionList]`
- 实现存储 `HashSet<SignedInclusionList>`

**影响**: 
- 可能导致 equivocation 检测逻辑不正确
- 内存和存储效率受影响

**修复方案**: 将 `SignedInclusionList` 改为 `InclusionList`。

---

### 3. process_inclusion_list 参数类型错误

**文件**: `beacon_node/beacon_chain/src/inclusion_list_verification.rs`

**问题**: 函数签名与规范不一致。

**规范定义**:
```python
def process_inclusion_list(
    store: InclusionListStore, 
    inclusion_list: InclusionList,  # 注意：不是 SignedInclusionList
    is_before_view_freeze_cutoff: bool
) -> None:
```

**当前实现**:
```rust
pub fn process_inclusion_list(
    &self,
    signed_il: SignedInclusionList<E>,  // 错误：应该是 InclusionList
    is_before_view_freeze_cutoff: bool,
) -> bool
```

**影响**: 验证逻辑可能不正确。

---

## P1 重要问题

### 4. core_topics_to_subscribe 未添加 IL 主题

**文件**: `beacon_node/lighthouse_network/src/types/topics.rs`

**问题**: 在 `core_topics_to_subscribe` 函数中，没有为 Heze fork 添加 `SignedInclusionList` 主题。

**规范要求**:
```python
# specs/heze/p2p-interface.md
# Heze introduces a new global topic for inclusion lists.
```

**当前实现**: 
```rust
if fork_name.gloas_enabled() {
    topics.push(GossipKind::ExecutionPayload);
    // ...
}
// 缺少 Heze 的 IL 主题
```

**修复方案**:
```rust
if fork_name.heze_enabled() {
    topics.push(GossipKind::SignedInclusionList);
}
```

---

### 5. P2P 验证规则不完整

**文件**: `beacon_node/beacon_chain/src/inclusion_list_verification.rs`

**问题**: 缺少部分 P2P 验证规则。

**规范要求** (specs/heze/p2p-interface.md):
1. ✅ `MAX_BYTES_PER_INCLUSION_LIST` 检查
2. ✅ Slot 是当前或前一个 slot
3. ❌ `inclusion_list_committee_root` 匹配验证
4. ❌ 验证者在委员会中的检查
5. ❌ 每个验证者最多 2 个 IL 的检查
6. ✅ 签名验证

**缺失的验证**:
```rust
// 需要添加:
// - [IGNORE] committee_root 匹配检查
// - [REJECT] 验证者在委员会中
// - [IGNORE] 每个验证者最多 2 个 IL
```

---

### 6. get_inclusion_list_bits 返回类型问题

**文件**: `beacon_node/beacon_chain/src/inclusion_list_verification.rs`

**问题**: 实现与规范有细微差异。

**规范**:
```python
def get_inclusion_list_bits(
    store: InclusionListStore, state: BeaconState, slot: Slot
) -> Bitvector[INCLUSION_LIST_COMMITTEE_SIZE]:
    # ...
    return Bitvector[INCLUSION_LIST_COMMITTEE_SIZE](
        validator_index in validator_indices for validator_index in inclusion_list_committee
    )
```

**当前实现**: 需要验证 Bitvector 的创建逻辑是否正确匹配委员会索引。

---

### 7. 缺少 heze_enabled() 辅助方法

**文件**: `consensus/types/src/fork/fork_name.rs`

**问题**: ForkName 缺少 `heze_enabled()` 方法，导致代码中无法方便地检查 Heze fork。

**需要添加**:
```rust
pub fn heze_enabled(self) -> bool {
    self >= ForkName::Heze
}
```

---

## P2 建议改进

### 8. Engine API 端点未实现

**文件**: `beacon_node/execution_layer/src/engine_api/`

**问题**: 只定义了 JSON 结构，没有实际的 API 调用逻辑。

**缺失**:
- `engine_getInclusionListV1` 实际调用
- `engine_newInclusionListV1` 实际调用
- 与 EL 的集成逻辑

---

### 9. Beacon API 只是占位符

**文件**: `beacon_node/http_api/src/inclusion_list.rs`

**问题**: 所有端点都是 stub，没有实际业务逻辑。

**需要实现**:
- `get_inclusion_lists`: 获取存储的 IL
- `get_inclusion_list_committee`: 返回委员会信息
- `submit_inclusion_list`: 处理 IL 提交

---

### 10. 缺少单元测试

**问题**: 大部分代码缺少充分的单元测试覆盖。

**建议**:
- 添加 `get_inclusion_list_committee` 测试
- 添加 equivocation 检测测试
- 添加 P2P 验证规则测试

---

## 正确实现的部分 ✅

### FOCIL-001: SSZ 类型定义
- ✅ `InclusionList` 结构正确
- ✅ `SignedInclusionList` 结构正确
- ✅ `MAX_BYTES_PER_INCLUSION_LIST = 8192`
- ✅ `SignedRoot` trait 实现

### FOCIL-002: Domain 和常量
- ✅ `DOMAIN_INCLUSION_LIST_COMMITTEE = 0x0E000000` (14)
- ✅ `INCLUSION_LIST_COMMITTEE_SIZE = 16`

### FOCIL-002: 委员会选择算法
- ✅ `get_inclusion_list_committee` 实现正确
- ✅ `get_inclusion_list_committee_root` 实现正确
- ✅ `is_valid_inclusion_list_signature` 实现正确

### FOCIL-003: P2P Gossip 主题
- ✅ `GossipKind::SignedInclusionList` 已添加
- ✅ `SIGNED_INCLUSION_LIST_TOPIC` 常量定义

### FOCIL-004: 验证框架
- ✅ `VerifiedInclusionList` 结构
- ✅ `verify_basic()` 方法

### FOCIL-006: InclusionListStore 核心逻辑
- ✅ `process_inclusion_list` 主要逻辑
- ✅ `get_inclusion_list_transactions`
- ✅ `is_inclusive`
- ✅ `prune` 清理逻辑

---

## 修复优先级建议

1. **立即修复 (P0)**:
   - 添加 `ForkName::Heze`
   - 修复 `InclusionListStore` 类型
   - 修复 `process_inclusion_list` 参数

2. **尽快修复 (P1)**:
   - 添加 `heze_enabled()` 方法
   - 更新 `core_topics_to_subscribe`
   - 完善 P2P 验证规则

3. **后续完善 (P2)**:
   - 实现 Engine API 调用
   - 实现 Beacon API 业务逻辑
   - 添加测试覆盖

---

## 参考规范

- [EIP-7805](https://eips.ethereum.org/EIPS/eip-7805)
- [Heze Beacon Chain](../eth2030/refs/consensus-specs/specs/heze/beacon-chain.md)
- [Heze P2P Interface](../eth2030/refs/consensus-specs/specs/heze/p2p-interface.md)
- [Heze Fork Choice](../eth2030/refs/consensus-specs/specs/heze/fork-choice.md)
- [Heze Inclusion List](../eth2030/refs/consensus-specs/specs/heze/inclusion-list.md)