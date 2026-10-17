Nat Han Adventures: the level editor and the music editor
==========================================================

level-editor   paint levels, prove them with Han, play them (F5), copy a share link
music-editor   the game's band: edit its songs as a tracker, a piano roll or text

The first time either one runs, it makes a folder "Nat Han Adventures" in your Documents
with the game's levels and songs in it. Your own levels go in its levels/custom folder.
(Run an editor from a folder that has levels/ or music/ in it and it uses that one instead.)

macOS: the editors aren't signed. The first time, right-click (or Control-click) the file
and choose Open, then Open again. Or, in Terminal: xattr -d com.apple.quarantine level-editor

Linux: needs the usual desktop sound and input libraries (libasound2, libudev1) and a
graphics driver with Vulkan.

Windows: if SmartScreen stops it, click "More info", then "Run anyway".
