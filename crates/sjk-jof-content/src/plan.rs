//! Which groups and files the player wants, and what a sync does about it.

use crate::store::{AssetState, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

/// What the player chose: the groups (`jof.contentGroups`), the repository's
/// defaults until the player changes one, then the ids chosen; and single files
/// picked against their group's tick (`jof.contentFiles`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    chosen: Option<BTreeSet<u32>>,
    /// Asset id: wanted or not, where it differs from its group's tick.
    files: BTreeMap<u32, bool>,
}

/// The setting's value before any choice.
pub const DEFAULT: &str = "default";

impl Selection {
    /// `default`, `none`, or group ids separated by spaces or commas.
    pub fn parse(value: &str) -> Self {
        let value = value.trim();
        if value.is_empty() || value.eq_ignore_ascii_case(DEFAULT) {
            return Self::default();
        }
        Self {
            chosen: Some(
                value
                    .split([' ', ','])
                    .filter_map(|id| id.trim().parse().ok())
                    .collect(),
            ),
            files: BTreeMap::new(),
        }
    }

    /// The setting's value.
    pub fn saved(&self) -> String {
        match &self.chosen {
            None => DEFAULT.to_owned(),
            Some(ids) if ids.is_empty() => "none".to_owned(),
            Some(ids) => ids.iter().map(u32::to_string).collect::<Vec<_>>().join(" "),
        }
    }

    /// Add the single-file picks: `+<id>` wanted, `-<id>` left out, separated
    /// by spaces or commas.
    pub fn with_files(mut self, value: &str) -> Self {
        self.files = value
            .split([' ', ','])
            .filter_map(|pick| {
                let pick = pick.trim();
                let wanted = match pick.chars().next()? {
                    '+' => true,
                    '-' => false,
                    _ => return None,
                };
                Some((pick[1..].parse().ok()?, wanted))
            })
            .collect();
        self
    }

    /// The single-file picks' setting value (empty without any).
    pub fn saved_files(&self) -> String {
        self.files
            .iter()
            .map(|(id, wanted)| format!("{}{id}", if *wanted { '+' } else { '-' }))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Whether one file of `group` is wanted: its own pick, else the group's tick.
    pub fn wants(&self, group: &crate::Group, asset: &crate::Asset) -> bool {
        self.files
            .get(&asset.id)
            .copied()
            .unwrap_or_else(|| self.chosen(group))
    }

    /// How many of the group's files for SJK are wanted.
    pub fn wanted_in(&self, group: &crate::Group) -> usize {
        group
            .assets
            .iter()
            .filter(|asset| asset.target == crate::Target::Base && self.wants(group, asset))
            .count()
    }

    /// Pick or leave out one file; a pick that matches its group's tick is
    /// forgotten.
    pub fn toggle_file(&mut self, group: &crate::Group, asset: &crate::Asset) {
        let wanted = !self.wants(group, asset);
        if wanted == self.chosen(group) {
            self.files.remove(&asset.id);
        } else {
            self.files.insert(asset.id, wanted);
        }
    }

    pub fn chosen(&self, group: &crate::Group) -> bool {
        match &self.chosen {
            None => group.kind.default_chosen(),
            Some(ids) => ids.contains(&group.id),
        }
    }

    /// Choose or leave out one whole group; the others keep what they show now.
    /// A group with every file wanted is left out, any other is chosen whole;
    /// either way its single-file picks are forgotten.
    pub fn toggle(&mut self, manifest: &crate::Manifest, group: u32) {
        let mut ids: BTreeSet<u32> = manifest
            .groups
            .iter()
            .filter(|known| self.chosen(known))
            .map(|known| known.id)
            .collect();
        if let Some(known) = manifest.group(group) {
            let all = known
                .assets
                .iter()
                .filter(|asset| asset.target == crate::Target::Base)
                .all(|asset| self.wants(known, asset));
            if all {
                ids.remove(&group);
            } else {
                ids.insert(group);
            }
            for asset in &known.assets {
                self.files.remove(&asset.id);
            }
        }
        self.chosen = Some(ids);
    }
}

/// The groups an automatic update keeps up to date (`jof.contentAutoGroups`,
/// `sith.cloudAutoGroups`): every group the player takes files from (`all`, the
/// default), or the group ids listed (empty for none). The player's choice of
/// files ([`Selection`]) still says which files of a group are wanted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AutoGroups {
    #[default]
    All,
    Only(BTreeSet<u32>),
}

/// The automatic groups' setting before any choice.
pub const AUTO_ALL: &str = "all";

impl AutoGroups {
    /// `all`, or group ids separated by spaces or commas (empty or `none`: no group).
    pub fn parse(value: &str) -> Self {
        let value = value.trim();
        if value.eq_ignore_ascii_case(AUTO_ALL) {
            return Self::All;
        }
        Self::Only(
            value
                .split([' ', ','])
                .filter_map(|id| id.trim().parse().ok())
                .collect(),
        )
    }

    /// The setting's value.
    pub fn saved(&self) -> String {
        match self {
            Self::All => AUTO_ALL.to_owned(),
            Self::Only(ids) => ids.iter().map(u32::to_string).collect::<Vec<_>>().join(" "),
        }
    }

    /// Whether `group` updates by itself.
    pub fn includes(&self, selection: &Selection, group: &crate::Group) -> bool {
        match self {
            Self::All => selection.chosen(group) || selection.wanted_in(group) > 0,
            Self::Only(ids) => ids.contains(&group.id),
        }
    }

    /// The same for a group known by its id only, which the manifest may no
    /// longer list: under `all`, a group gone from it counts (the player took
    /// its files while it was listed).
    pub fn includes_id(&self, selection: &Selection, manifest: &crate::Manifest, id: u32) -> bool {
        match self {
            Self::All => manifest
                .group(id)
                .is_none_or(|group| self.includes(selection, group)),
            Self::Only(ids) => ids.contains(&id),
        }
    }

    /// Mark or unmark one group; under `all` the others keep what they show now.
    pub fn toggle(&mut self, selection: &Selection, manifest: &crate::Manifest, id: u32) {
        let mut ids: BTreeSet<u32> = manifest
            .groups
            .iter()
            .filter(|group| self.includes(selection, group))
            .map(|group| group.id)
            .collect();
        if let Self::Only(known) = self {
            // Groups the manifest no longer lists keep their mark.
            ids.extend(known.iter().copied());
        }
        if !ids.remove(&id) {
            ids.insert(id);
        }
        *self = Self::Only(ids);
    }
}

/// What a sync will do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// `(group, asset)` indices of the files to fetch, smallest first so the
    /// most files are ready soonest.
    pub downloads: Vec<(usize, usize)>,
    pub download_bytes: u64,
    /// File names to delete from SJK's JoF folder.
    pub removals: Vec<String>,
    pub removal_bytes: u64,
}

impl Plan {
    /// Fetch the wanted files that are missing; delete the files SJK downloaded
    /// that are wanted nowhere, and its strays. Files in the game's folder, and
    /// files put in SJK's folder by hand, are never touched.
    pub fn new(snapshot: &Snapshot, selection: &Selection) -> Self {
        Self::make(snapshot, selection, None)
    }

    /// The same plan restricted to the `auto` groups, for an automatic update:
    /// fetch their wanted files that are missing or outdated, and delete only
    /// the files SJK downloaded for one of them that the manifest no longer
    /// lists. Files the player left out are not removed (the player's own
    /// Download does that), and the other groups are not touched.
    pub fn auto(snapshot: &Snapshot, selection: &Selection, auto: &AutoGroups) -> Self {
        Self::make(snapshot, selection, Some(auto))
    }

    fn make(snapshot: &Snapshot, selection: &Selection, auto: Option<&AutoGroups>) -> Self {
        let mut plan = Plan::default();
        let kept: BTreeSet<String> = snapshot
            .manifest
            .assets()
            .filter(|(group, asset)| selection.wants(group, asset))
            .map(|(_, asset)| asset.name.to_ascii_lowercase())
            .collect();
        let mut fetched = BTreeSet::new();
        for (group_index, group) in snapshot.manifest.groups.iter().enumerate() {
            if auto.is_some_and(|auto| !auto.includes(selection, group)) {
                continue;
            }
            for (asset_index, asset) in group.assets.iter().enumerate() {
                let chosen = selection.wants(group, asset);
                let name = asset.name.to_ascii_lowercase();
                match snapshot.state(group_index, asset_index) {
                    AssetState::Missing | AssetState::Outdated
                        if chosen && fetched.insert(name.clone()) =>
                    {
                        plan.downloads.push((group_index, asset_index));
                        plan.download_bytes += asset.size;
                    }
                    AssetState::Installed | AssetState::Outdated
                        if auto.is_none()
                            && !kept.contains(&name)
                            && snapshot.downloaded.contains(&name)
                            && !plan
                                .removals
                                .iter()
                                .any(|known| known.eq_ignore_ascii_case(&name)) =>
                    {
                        plan.removals.push(asset.name.clone());
                        plan.removal_bytes += asset.size;
                    }
                    _ => {}
                }
            }
        }
        let listed = |name: &str| {
            snapshot
                .manifest
                .assets()
                .any(|(_, asset)| asset.name.eq_ignore_ascii_case(name))
        };
        for (name, size) in &snapshot.strays {
            if let Some(auto) = auto {
                let ours = snapshot
                    .origins
                    .get(&name.to_ascii_lowercase())
                    .is_some_and(|&group| auto.includes_id(selection, &snapshot.manifest, group));
                if !ours || listed(name) {
                    continue;
                }
            }
            plan.removals.push(name.clone());
            plan.removal_bytes += size;
        }
        let manifest = &snapshot.manifest;
        plan.downloads
            .sort_by_key(|&(group, asset)| manifest.groups[group].assets[asset].size);
        plan
    }

    pub fn is_empty(&self) -> bool {
        self.downloads.is_empty() && self.removals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::manifest;

    fn snapshot(states: Vec<Vec<AssetState>>) -> Snapshot {
        Snapshot {
            manifest: manifest(),
            states,
            strays: vec![("gone.pk3".into(), 9)],
            downloaded: ["c.pk3".to_owned()].into(),
            origins: BTreeMap::new(),
        }
    }

    #[test]
    fn the_defaults_choose_all_but_optional_groups_and_save_once_changed() {
        let manifest = manifest();
        let mut selection = Selection::parse("default");
        assert!(selection.chosen(&manifest.groups[0]));
        assert!(!selection.chosen(&manifest.groups[1]));
        assert_eq!(selection.saved(), "default");
        selection.toggle(&manifest, 2);
        assert_eq!(selection.saved(), "1 2");
        selection.toggle(&manifest, 1);
        selection.toggle(&manifest, 2);
        assert_eq!(selection.saved(), "none");
        assert_eq!(Selection::parse("none"), selection);
        assert_eq!(Selection::parse("2,1"), Selection::parse("1 2"));
    }

    #[test]
    fn a_sync_fetches_chosen_files_and_removes_only_its_own() {
        use AssetState::*;
        let snapshot = snapshot(vec![vec![Missing, InGameFolder], vec![Installed]]);
        let plan = Plan::new(&snapshot, &Selection::parse("1"));
        assert_eq!(plan.downloads, [(0, 0)]);
        assert_eq!(plan.download_bytes, 3);
        assert_eq!(plan.removals, ["c.pk3", "gone.pk3"]);
        assert_eq!(plan.removal_bytes, 2 + 9);

        let by_hand = Snapshot {
            downloaded: BTreeSet::new(),
            ..snapshot.clone()
        };
        assert_eq!(
            Plan::new(&by_hand, &Selection::parse("1")).removals,
            ["gone.pk3"],
            "c.pk3 was put there by hand"
        );

        let plan = Plan::new(&snapshot, &Selection::parse("none"));
        assert!(plan.downloads.is_empty());
        assert_eq!(
            plan.removals,
            ["c.pk3", "gone.pk3"],
            "the game folder's b.pk3 is left alone"
        );

        let snapshot = snapshot_without_strays(vec![vec![Outdated, Missing], vec![Missing]]);
        let plan = Plan::new(&snapshot, &Selection::parse("1 2"));
        assert_eq!(plan.downloads, [(1, 0), (0, 0), (0, 1)], "smallest first");
        assert!(plan.removals.is_empty());
    }

    #[test]
    fn single_files_are_picked_against_their_group() {
        use AssetState::*;
        let manifest = manifest();
        let (one, two) = (&manifest.groups[0], &manifest.groups[1]);
        let mut selection = Selection::default();
        // Leave b.pk3 out of a chosen group, take c.pk3 from an unchosen one.
        selection.toggle_file(one, &one.assets[1]);
        selection.toggle_file(two, &two.assets[0]);
        assert_eq!(selection.saved_files(), "-11 +20");
        assert_eq!(selection.wanted_in(one), 1);
        let again = Selection::parse(&selection.saved()).with_files(&selection.saved_files());
        assert_eq!(again, selection);

        let snapshot = snapshot_without_strays(vec![vec![Missing, Missing], vec![Missing]]);
        let plan = Plan::new(&snapshot, &selection);
        assert_eq!(plan.downloads, [(1, 0), (0, 0)]);

        // Picking b.pk3 back matches the group's tick: the pick is forgotten.
        selection.toggle_file(one, &one.assets[1]);
        assert_eq!(selection.saved_files(), "+20");
        // A fully wanted group's tick leaves it out, its pick forgotten.
        selection.toggle(&manifest, 2);
        assert_eq!(selection.saved_files(), "");
        assert_eq!(selection.wanted_in(two), 0);
        // A partly picked group is chosen whole by its tick.
        selection.toggle_file(one, &one.assets[0]);
        assert_eq!(selection.wanted_in(one), 1);
        selection.toggle(&manifest, 1);
        assert_eq!(
            (selection.wanted_in(one), selection.saved_files()),
            (2, String::new())
        );
    }

    #[test]
    fn automatic_groups_parse_save_and_follow_the_choice() {
        let manifest = manifest();
        let selection = Selection::parse("default");
        let mut auto = AutoGroups::parse(" ALL ");
        assert_eq!(auto, AutoGroups::All);
        assert!(auto.includes(&selection, &manifest.groups[0]));
        assert!(
            !auto.includes(&selection, &manifest.groups[1]),
            "all means the groups the player takes files from"
        );
        assert!(
            auto.includes_id(&selection, &manifest, 9),
            "a group gone from the list"
        );
        // Marking one under all keeps the others as they show.
        auto.toggle(&selection, &manifest, 2);
        assert_eq!(auto.saved(), "1 2");
        auto.toggle(&selection, &manifest, 1);
        assert_eq!(auto, AutoGroups::parse("2"));
        auto.toggle(&selection, &manifest, 2);
        assert_eq!(auto.saved(), "");
        assert_eq!(AutoGroups::parse(""), AutoGroups::parse("none"));
        assert!(!auto.includes_id(&selection, &manifest, 9));
    }

    #[test]
    fn an_automatic_update_keeps_to_its_groups() {
        use AssetState::*;
        let mut snapshot = snapshot(vec![vec![Outdated, Missing], vec![Missing]]);
        // Downloaded by SJK: gone.pk3 for group 1, old.pk3 for group 2, kept.pk3
        // before groups were noted.
        snapshot.strays = vec![
            ("gone.pk3".into(), 9),
            ("kept.pk3".into(), 5),
            ("old.pk3".into(), 4),
        ];
        snapshot.origins = [("gone.pk3".to_owned(), 1), ("old.pk3".to_owned(), 2)].into();
        let selection = Selection::parse("1 2");
        let plan = Plan::auto(&snapshot, &selection, &AutoGroups::parse("1"));
        assert_eq!(plan.downloads, [(0, 0), (0, 1)], "group 2's c.pk3 waits");
        assert_eq!(plan.removals, ["gone.pk3"]);
        assert_eq!(plan.removal_bytes, 9);

        let plan = Plan::auto(&snapshot, &selection, &AutoGroups::All);
        assert_eq!(plan.downloads.len(), 3);
        assert_eq!(plan.removals, ["gone.pk3", "old.pk3"]);

        assert!(Plan::auto(&snapshot, &selection, &AutoGroups::parse("")).is_empty());

        // A file the player left out is not removed by an automatic update.
        let snapshot = snapshot_without_strays(vec![vec![Installed, Installed], vec![Installed]]);
        let selection = Selection::parse("1");
        assert_eq!(Plan::new(&snapshot, &selection).removals, ["c.pk3"]);
        assert!(Plan::auto(&snapshot, &selection, &AutoGroups::parse("1 2")).is_empty());
    }

    fn snapshot_without_strays(states: Vec<Vec<AssetState>>) -> Snapshot {
        Snapshot {
            strays: Vec::new(),
            ..snapshot(states)
        }
    }
}
