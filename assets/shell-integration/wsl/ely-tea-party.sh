# Ely Tea Party: start this distribution's login shell with Ely Tea Party's integration, which WSLENV hands over as ELY_WSL.
case ${SHELL##*/} in
  zsh)
    [ -n "${ZDOTDIR+set}" ] && export ELY_ZSH_ZDOTDIR="$ZDOTDIR"
    export ZDOTDIR="$ELY_WSL/zsh"
    exec "$SHELL" -l
    ;;
  bash)
    export ELY_BASH_LOGIN=1
    exec "$SHELL" --rcfile "$ELY_WSL/bash/ely-tea-party.bash"
    ;;
  fish)
    export ELY_FISH_XDG_DATA_DIRS="${XDG_DATA_DIRS-}"
    export XDG_DATA_DIRS="$ELY_WSL:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
    exec "$SHELL" -l
    ;;
esac
printf 'Ely Tea Party: no integration for %s; its folder goes unreported\n' "${SHELL:?Ely Tea Party: WSL names no login shell}" >&2
exec "$SHELL" -l
