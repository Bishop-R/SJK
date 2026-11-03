// The Fishing toy's rod, bucket, float and line, and the fish's eyes and whiskers (SJK;
// see README.md): flat colours from one small atlas, lit by the world's light as a
// player model is, seen from both sides so the open bucket and the thin line never show
// holes.
models/sjk/fishing
{
	cull none
	{
		map models/sjk/fishing
		rgbGen lightingDiffuse
	}
}

// The fish: each one's own painted skin and fins from the fish atlas, lit the same way,
// both sides drawn for the thin fins.
models/sjk/fishing_fish
{
	cull none
	{
		map models/sjk/fishing_fish
		rgbGen lightingDiffuse
	}
}

// The pond, drawn with the installed game's own swamp textures (only their names are
// here: they come from the player's base assets, as a map's do). Its water is the
// stock dark water under two of Hidden Evil's lake layers, drifting as on that map.
models/sjk/fishing_pond_water
{
	cull none
	{
		map textures/common/water_1
		rgbGen lightingDiffuse
		tcMod turb 0.5 0.04 0 0.3
	}
	{
		map textures/h_evil/wf3
		blendFunc GL_SRC_ALPHA GL_ONE_MINUS_SRC_ALPHA
		rgbGen lightingDiffuse
		alphaGen const 0.16
		tcMod scroll 0.03 -0.13
	}
	{
		map textures/h_evil/wfn2
		blendFunc GL_SRC_ALPHA GL_ONE_MINUS_SRC_ALPHA
		rgbGen lightingDiffuse
		alphaGen const 0.12
		tcMod turb 1 0.02 0 0.15
		tcMod scroll -0.03 -0.1
	}
}

// The bank: Yavin's mossy mud.
models/sjk/fishing_pond_bank
{
	{
		map textures/yavin/s_mud2
		rgbGen lightingDiffuse
	}
}

// The stones: Yavin's mossy rock.
models/sjk/fishing_pond_stone
{
	{
		map textures/yavin/s_rock4
		rgbGen lightingDiffuse
	}
}

// Leaves floating on the water and lying on the bank: Yavin's ground plants.
models/sjk/fishing_pond_leaves
{
	cull none
	{
		map textures/yavin/groundplants
		alphaFunc GE128
		rgbGen lightingDiffuse
	}
}
