# Ely Tea Party: restore XDG_DATA_DIRS, report the folder; fish_title keeps the title.
if test -n "$ELY_FISH_XDG_DATA_DIRS"
    set -gx XDG_DATA_DIRS $ELY_FISH_XDG_DATA_DIRS
else
    set -e XDG_DATA_DIRS
end
set -e ELY_FISH_XDG_DATA_DIRS
if status is-interactive
    function _ely_prompt --on-event fish_prompt
        printf '\e]7;file://%s\a' (string escape --style=url -- $PWD)
    end
end
