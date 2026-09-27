use crate::core::R;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Group,
    Panel,
    Navigation,
    Toolbar,
    Button,
    Checkbox,
    Switch,
    Radio,
    RadioGroup,
    Tab,
    TabList,
    TabPanel,
    Menu,
    MenuItem,
    Separator,
    Slider,
    TextInput,
    Link,
    Text,
    Image,
    List,
    ListItem,
    ScrollArea,
    Tooltip,
    Dialog,
    Badge,
    Region,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Group => "group",
            Role::Panel => "panel",
            Role::Navigation => "navigation",
            Role::Toolbar => "toolbar",
            Role::Button => "button",
            Role::Checkbox => "checkbox",
            Role::Switch => "switch",
            Role::Radio => "radio",
            Role::RadioGroup => "radiogroup",
            Role::Tab => "tab",
            Role::TabList => "tablist",
            Role::TabPanel => "tabpanel",
            Role::Menu => "menu",
            Role::MenuItem => "menuitem",
            Role::Separator => "separator",
            Role::Slider => "slider",
            Role::TextInput => "textbox",
            Role::Link => "link",
            Role::Text => "text",
            Role::Image => "image",
            Role::List => "list",
            Role::ListItem => "listitem",
            Role::ScrollArea => "scrollarea",
            Role::Tooltip => "tooltip",
            Role::Dialog => "dialog",
            Role::Badge => "badge",
            Role::Region => "region",
        }
    }

    pub const ALL: [Role; 27] = [
        Role::Group,
        Role::Panel,
        Role::Navigation,
        Role::Toolbar,
        Role::Button,
        Role::Checkbox,
        Role::Switch,
        Role::Radio,
        Role::RadioGroup,
        Role::Tab,
        Role::TabList,
        Role::TabPanel,
        Role::Menu,
        Role::MenuItem,
        Role::Separator,
        Role::Slider,
        Role::TextInput,
        Role::Link,
        Role::Text,
        Role::Image,
        Role::List,
        Role::ListItem,
        Role::ScrollArea,
        Role::Tooltip,
        Role::Dialog,
        Role::Badge,
        Role::Region,
    ];

    pub fn parse(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == name)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct States(pub u16);

impl States {
    pub const NONE: States = States(0);
    pub const HOVERED: States = States(1);
    pub const FOCUSED: States = States(1 << 1);
    pub const PRESSED: States = States(1 << 2);
    pub const SELECTED: States = States(1 << 3);
    pub const CHECKED: States = States(1 << 4);
    pub const MIXED: States = States(1 << 5);
    pub const DISABLED: States = States(1 << 6);
    pub const EXPANDED: States = States(1 << 7);
    pub const BUSY: States = States(1 << 8);
    pub const INVALID: States = States(1 << 9);
    pub const HIDDEN: States = States(1 << 10);

    const NAMES: [(States, &'static str); 11] = [
        (States::HOVERED, "hovered"),
        (States::FOCUSED, "focused"),
        (States::PRESSED, "pressed"),
        (States::SELECTED, "selected"),
        (States::CHECKED, "checked"),
        (States::MIXED, "mixed"),
        (States::DISABLED, "disabled"),
        (States::EXPANDED, "expanded"),
        (States::BUSY, "busy"),
        (States::INVALID, "invalid"),
        (States::HIDDEN, "hidden"),
    ];

    pub fn contains(self, other: States) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn with(self, other: States, on: bool) -> States {
        if on { States(self.0 | other.0) } else { self }
    }

    pub fn names(self) -> impl Iterator<Item = &'static str> {
        States::NAMES.into_iter().filter(move |(s, _)| self.contains(*s)).map(|(_, n)| n)
    }

    pub fn parse(name: &str) -> Option<States> {
        States::NAMES.into_iter().find(|(_, n)| *n == name).map(|(s, _)| s)
    }
}

impl std::ops::BitOr for States {
    type Output = States;
    fn bitor(self, o: States) -> States {
        States(self.0 | o.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Number { value: f32, min: f32, max: f32 },
    Index(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueRef<'a> {
    Text(&'a str),
    Number { value: f32, min: f32, max: f32 },
    Index(usize),
}

impl ValueRef<'_> {
    pub fn owned(self) -> Value {
        match self {
            ValueRef::Text(t) => Value::Text(t.to_string()),
            ValueRef::Number { value, min, max } => Value::Number { value, min, max },
            ValueRef::Index(i) => Value::Index(i),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: u64,
    pub parent: Option<usize>,
    pub role: Role,
    pub name: String,
    pub value: Option<Value>,
    pub states: States,
    pub rect: R,
}

#[derive(Clone, Copy, Debug)]
pub struct Describe<'a> {
    pub id: u64,
    pub role: Role,
    pub rect: R,
    pub name: &'a str,
    pub states: States,
    pub value: Option<ValueRef<'a>>,
}

impl<'a> Describe<'a> {
    pub fn new(id: u64, role: Role, rect: R) -> Describe<'a> {
        Describe {
            id,
            role,
            rect,
            name: "",
            states: States::NONE,
            value: None,
        }
    }

    pub fn name(mut self, name: &'a str) -> Self {
        self.name = name;
        self
    }

    pub fn state(mut self, s: States, on: bool) -> Self {
        self.states = self.states.with(s, on);
        self
    }

    pub fn value(mut self, v: ValueRef<'a>) -> Self {
        self.value = Some(v);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_combine_and_list_their_names_in_a_fixed_order() {
        let s = States::SELECTED | States::HOVERED;
        assert!(s.contains(States::HOVERED) && !s.contains(States::DISABLED));
        assert_eq!(s.names().collect::<Vec<_>>(), ["hovered", "selected"]);
        assert_eq!(States::parse("busy"), Some(States::BUSY));
    }

    #[test]
    fn roles_round_trip_through_their_names() {
        for r in Role::ALL {
            assert_eq!(Role::parse(r.name()), Some(r));
        }
    }
}
