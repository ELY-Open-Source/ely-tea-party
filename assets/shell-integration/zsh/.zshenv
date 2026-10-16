# Ely Tea Party: restore ZDOTDIR, load the user's .zshenv, report folder and title.
if [[ -n "${ELY_ZSH_ZDOTDIR+set}" ]]; then
  ZDOTDIR=$ELY_ZSH_ZDOTDIR
  builtin unset ELY_ZSH_ZDOTDIR
else
  builtin unset ZDOTDIR
fi
[[ -r "${ZDOTDIR:-$HOME}/.zshenv" ]] && builtin source "${ZDOTDIR:-$HOME}/.zshenv"
if [[ -o interactive ]]; then
  _ely_precmd() {
    builtin local ret=$? LC_ALL=C char url=
    for char in ${(s::)PWD}; do
      case $char in
        [[:cntrl:]]|%) builtin printf -v char '%%%02X' "'$char" ;;
      esac
      url+=$char
    done
    builtin printf '\e]7;file://%s\a\e]2;%s\a' "$url" "${${${PWD:t}:-/}//[[:cntrl:]]/}"
    builtin return ret
  }
  _ely_preexec() {
    builtin printf '\e]2;%s\a' "${1//[[:cntrl:]]/}"
  }
  builtin autoload -Uz add-zsh-hook
  add-zsh-hook precmd _ely_precmd
  add-zsh-hook preexec _ely_preexec
fi
