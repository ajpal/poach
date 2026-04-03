//! Mechanisms for declaring types of base values and functions on them.

use std::{
    any::{Any, TypeId},
    fmt::{self, Debug, Display},
    hash::Hash,
};

use hashbrown::HashMap;
use num::{rational::Ratio, BigInt, Rational64};
use serde::{de, ser::SerializeStruct, Deserialize, Serialize};

use crate::numeric_id::{define_id, DenseIdMap, NumericId};

use crate::common::{InternTable, Value};

#[cfg(test)]
mod tests;
mod unboxed;

define_id!(pub BaseValueId, u32, "an identifier for base value types");

/// A simple data type that can be interned in a database.
///
/// Most callers can simply implement this trait on their desired type, with no overrides needed.
/// For types that are particularly small, users can override [`BaseValue::try_box`] and [`BaseValue::try_unbox`]
/// methods and set [`BaseValue::MAY_UNBOX`] to `true` to allow the Rust value to be stored in-place in a
/// [`Value`].
///
/// Regardless, all base value types should be registered in a [`BaseValues`] instance using the
/// [`BaseValues::register_type`] method before they can be used in the database.
pub trait BaseValue:
// deserialize needed here for InternTable::deserialize
    Clone + Hash + Eq + Any + Debug + Send + Sync + Serialize + for<'de> Deserialize<'de>
{
    const MAY_UNBOX: bool = false;
    fn intern(&self, table: &InternTable<Self, Value>) -> Value {
        table.intern(self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn try_box(&self) -> Option<Value> {
        None
    }
    fn try_unbox(_val: Value) -> Option<Self> {
        None
    }

    fn type_id_string() -> String;
}

impl BaseValue for String {
    fn type_id_string() -> String {
        "String".into()
    }
}

impl BaseValue for num::Rational64 {
    fn type_id_string() -> String {
        "Rational64".into()
    }
}

/// A wrapper used to print a base value.
///
/// The given base value type must be registered with the [`BaseValues`] instance,
/// otherwise attempting to call the [`Debug`] implementation will panic.
pub struct BaseValuePrinter<'a> {
    pub base: &'a BaseValues,
    pub ty: BaseValueId,
    pub val: Value,
}

impl Debug for BaseValuePrinter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.base.tables[self.ty].print_value(self.val, f)
    }
}

/// A registry for base value types and functions on them.
#[derive(Clone, Default)]
pub struct BaseValues {
    type_ids: HashMap<TypeId, BaseValueId>,
    tables: DenseIdMap<BaseValueId, Box<dyn DynamicInternTable>>,
    pending_tables: HashMap<String, (BaseValueId, Vec<u8>)>,
}

impl<'de> Deserialize<'de> for BaseValues {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Partial {
            tables: Vec<(BaseValueId, BaseInternTableErased)>,
        }

        let partial = Partial::deserialize(deserializer)?;

        let mut tables = DenseIdMap::default();
        let mut type_ids = HashMap::new();
        let mut pending_tables = HashMap::new();
        for (id, value) in partial.tables {
            match deserialize_dyn(value) {
                Ok(DeserializeDynOutcome::Ready(table)) => {
                    let type_id = table.get_type_id();
                    type_ids.insert(type_id, id);
                    tables.insert(id, table);
                }
                Ok(DeserializeDynOutcome::Pending {
                    base_value_type,
                    table_bytes,
                }) => {
                    pending_tables.insert(base_value_type, (id, table_bytes));
                }
                Err(err) => {
                    return Err(de::Error::custom(err));
                }
            }
        }

        Ok(BaseValues {
            type_ids,
            tables,
            pending_tables,
        })
    }
}

impl Serialize for BaseValues {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("BaseValues", 1)?;

        let mut table_entries = Vec::new();
        for (id, table) in self.tables.iter() {
            let serialized_table = table.serialize_dyn().map_err(serde::ser::Error::custom)?;
            table_entries.push((id, serialized_table));
        }
        for (base_value_type, (id, table_bytes)) in &self.pending_tables {
            table_entries.push((
                *id,
                BaseInternTableErased {
                    table_bytes: table_bytes.clone(),
                    base_value_type: base_value_type.clone(),
                },
            ));
        }

        state.serialize_field("tables", &table_entries)?;
        state.end()
    }
}

impl Display for BaseValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BaseValues {{ types: {} }}", self.type_ids.len())
    }
}

impl BaseValues {
    /// Register the given type `P` as a base value type in this registry.
    pub fn register_type<P: BaseValue>(&mut self) -> BaseValueId {
        let type_id = TypeId::of::<P>();
        if let Some(id) = self.type_ids.get(&type_id) {
            return *id;
        }
        if let Some((id, table_bytes)) = self.pending_tables.remove(&P::type_id_string()) {
            let reader = flexbuffers::Reader::get_root(table_bytes.as_slice())
                .expect("pending base value table was not valid flexbuffers");
            let table: InternTable<P, Value> = InternTable::deserialize(reader)
                .expect("pending base value table did not match registered type");
            self.tables.insert(id, Box::new(BaseInternTable { table }));
            self.type_ids.insert(type_id, id);
            return id;
        }
        let next_id = BaseValueId::from_usize(self.type_ids.len());
        let id = *self.type_ids.entry(type_id).or_insert(next_id);
        self.tables
            .get_or_insert(id, || Box::<BaseInternTable<P>>::default());
        id
    }

    /// Get the [`BaseValueId`] for the given base value type `P`.
    pub fn get_ty<P: BaseValue>(&self) -> BaseValueId {
        self.type_ids[&TypeId::of::<P>()]
    }

    /// Get the [`BaseValueId`] for the given base value type id.
    pub fn get_ty_by_id(&self, id: TypeId) -> BaseValueId {
        self.type_ids[&id]
    }

    /// Get a [`Value`] representing the given base value `p`.
    pub fn get<P: BaseValue>(&self, p: P) -> Value {
        if P::MAY_UNBOX {
            if let Some(v) = p.try_box() {
                return v;
            }
        }
        let id = self.get_ty::<P>();
        let table = self.tables[id]
            .as_any()
            .downcast_ref::<BaseInternTable<P>>()
            .unwrap();
        table.intern(p)
    }

    /// Get the base value of type `P` corresponding to the given [`Value`].
    pub fn unwrap<P: BaseValue>(&self, v: Value) -> P {
        if P::MAY_UNBOX {
            if let Some(p) = P::try_unbox(v) {
                return p;
            }
        }
        let id = self.get_ty::<P>();
        let table = self
            .tables
            .get(id)
            .expect("types must be registered before unwrapping")
            .as_any()
            .downcast_ref::<BaseInternTable<P>>()
            .unwrap();
        table.get(v)
    }
}

trait DynamicInternTable: Any + dyn_clone::DynClone + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn print_value(&self, val: Value, f: &mut fmt::Formatter) -> fmt::Result;
    fn serialize_dyn(&self) -> Result<BaseInternTableErased, Box<dyn std::error::Error>>;
    fn get_type_id(&self) -> TypeId;
}

// Implements `Clone` for `Box<dyn DynamicInternTable>`.
dyn_clone::clone_trait_object!(DynamicInternTable);

#[derive(Serialize, Deserialize)]
struct BaseInternTableErased {
    table_bytes: Vec<u8>,
    base_value_type: String,
}

#[derive(Clone, Serialize)]
struct BaseInternTable<P: BaseValue> {
    table: InternTable<P, Value>,
}

impl<P: BaseValue> Default for BaseInternTable<P> {
    fn default() -> Self {
        Self {
            table: InternTable::default(),
        }
    }
}

impl<P> DynamicInternTable for BaseInternTable<P>
where
    P: BaseValue + Serialize + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn print_value(&self, val: Value, f: &mut fmt::Formatter) -> fmt::Result {
        let p = self.get(val);
        write!(f, "{p:?}")
    }

    fn serialize_dyn(&self) -> Result<BaseInternTableErased, Box<dyn std::error::Error>> {
        let mut serializer = flexbuffers::FlexbufferSerializer::new();
        self.table.serialize(&mut serializer)?;
        Ok(BaseInternTableErased {
            table_bytes: Vec::from(serializer.view()),
            base_value_type: self.get_type_id_string(),
        })
    }

    fn get_type_id(&self) -> TypeId {
        TypeId::of::<P>()
    }
}

const VAL_OFFSET: u32 = 1 << (std::mem::size_of::<Value>() as u32 * 8 - 1);

impl<P: BaseValue> BaseInternTable<P> {
    pub fn get_type_id_string(&self) -> String {
        P::type_id_string()
    }

    pub fn intern(&self, p: P) -> Value {
        if P::MAY_UNBOX {
            p.try_box().unwrap_or_else(|| {
                // If the base value type is too large to fit in a Value, we intern it and return
                // the corresponding Value with its top bit set. We use add to ensure we overflow
                // if the number of interned values is too large.
                Value::new(
                    self.table
                        .intern(&p)
                        .rep()
                        .checked_add(VAL_OFFSET)
                        .expect("interned value overflowed"),
                )
            })
        } else {
            self.table.intern(&p)
        }
    }

    pub fn get(&self, v: Value) -> P {
        if P::MAY_UNBOX {
            P::try_unbox(v)
                .unwrap_or_else(|| self.table.get_cloned(Value::new(v.rep() - VAL_OFFSET)))
        } else {
            self.table.get_cloned(v)
        }
    }
}

/// A newtype wrapper used to implement the [`BaseValue`] trait on types not
/// defined in this crate.
///
/// This type is just a helper: users can also implement the [`BaseValue`] trait directly on their
/// types if the type is defined in the crate in which the implementation is defined, or if they
/// need custom logic for boxing or unboxing the type.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Boxed<T>(pub T);

impl<T> Boxed<T> {
    pub fn new(value: T) -> Self {
        Boxed(value)
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: Debug> Debug for Boxed<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<
        T: Hash + Eq + Debug + Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    > BaseValue for Boxed<T>
{
    fn type_id_string() -> String {
        format!("Boxed<{}>", std::any::type_name::<T>())
    }
}

impl<T> std::ops::Deref for Boxed<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Boxed<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> From<T> for Boxed<T> {
    fn from(value: T) -> Self {
        Boxed(value)
    }
}

impl<T: Copy> From<&T> for Boxed<T> {
    fn from(value: &T) -> Self {
        Boxed(*value)
    }
}

impl<T: std::ops::Add<Output = T>> std::ops::Add for Boxed<T> {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Boxed(self.0 + other.0)
    }
}

impl<T: std::ops::Sub<Output = T>> std::ops::Sub for Boxed<T> {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Boxed(self.0 - other.0)
    }
}

impl<T: std::ops::Mul<Output = T>> std::ops::Mul for Boxed<T> {
    type Output = Self;

    fn mul(self, other: Self) -> Self::Output {
        Boxed(self.0 * other.0)
    }
}

impl<T: std::ops::Div<Output = T>> std::ops::Div for Boxed<T> {
    type Output = Self;

    fn div(self, other: Self) -> Self::Output {
        Boxed(self.0 / other.0)
    }
}

impl<T: std::ops::Rem<Output = T>> std::ops::Rem for Boxed<T> {
    type Output = Self;

    fn rem(self, other: Self) -> Self::Output {
        Boxed(self.0 % other.0)
    }
}

impl<T: std::ops::Neg<Output = T>> std::ops::Neg for Boxed<T> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Boxed(-self.0)
    }
}

impl<T: std::ops::BitAnd<Output = T>> std::ops::BitAnd for Boxed<T> {
    type Output = Self;

    fn bitand(self, other: Self) -> Self::Output {
        Boxed(self.0 & other.0)
    }
}

impl<T: std::ops::BitOr<Output = T>> std::ops::BitOr for Boxed<T> {
    type Output = Self;

    fn bitor(self, other: Self) -> Self::Output {
        Boxed(self.0 | other.0)
    }
}

impl<T: std::ops::BitXor<Output = T>> std::ops::BitXor for Boxed<T> {
    type Output = Self;

    fn bitxor(self, other: Self) -> Self::Output {
        Boxed(self.0 ^ other.0)
    }
}

fn deserialize_dyn(
    erased: BaseInternTableErased,
) -> Result<DeserializeDynOutcome, Box<dyn std::error::Error>> {
    fn parse_table<P: BaseValue>(
        table_bytes: &[u8],
    ) -> Result<InternTable<P, Value>, Box<dyn std::error::Error>> {
        let reader = flexbuffers::Reader::get_root(table_bytes)?;
        InternTable::deserialize(reader).map_err(Box::<dyn std::error::Error>::from)
    }

    match erased.base_value_type.as_str() {
        "Unit" => {
            let table: InternTable<(), Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "Bool" => {
            let table: InternTable<bool, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "String" => {
            let table: InternTable<String, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "StaticStr" => {
            panic!("can't deserialize static strings")
            // let table: InternTable<&'static str, Value> = parse_table(&erased.table_bytes)?;
            // Ok(Box::new(BaseInternTable { table }))
        }
        "Rational64" => {
            let table: InternTable<Rational64, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "u8" => {
            let table: InternTable<u8, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "u16" => {
            let table: InternTable<u16, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "u32" => {
            let table: InternTable<u32, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "u64" => {
            let table: InternTable<u64, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "usize" => {
            let table: InternTable<usize, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "i8" => {
            let table: InternTable<i8, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "i16" => {
            let table: InternTable<i16, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "i32" => {
            let table: InternTable<i32, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "i64" => {
            let table: InternTable<i64, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "isize" => {
            let table: InternTable<isize, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }

        "Boxed<alloc::string::String>" => {
            let table: InternTable<Boxed<String>, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "Boxed<ordered_float::OrderedFloat<f64>>" => {
            let table: InternTable<Boxed<ordered_float::OrderedFloat<f64>>, Value> =
                parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }
        "Boxed<num_bigint::bigint::BigInt>" => {
            let table: InternTable<Boxed<BigInt>, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }

        "Boxed<num_rational::Ratio<num_bigint::bigint::BigInt>>" => {
            let table: InternTable<Boxed<Ratio<BigInt>>, Value> = parse_table(&erased.table_bytes)?;
            Ok(DeserializeDynOutcome::Ready(Box::new(BaseInternTable {
                table,
            })))
        }

        _ => Ok(DeserializeDynOutcome::Pending {
            base_value_type: erased.base_value_type,
            table_bytes: erased.table_bytes,
        }),
    }
}

enum DeserializeDynOutcome {
    Ready(Box<dyn DynamicInternTable>),
    Pending {
        base_value_type: String,
        table_bytes: Vec<u8>,
    },
}
