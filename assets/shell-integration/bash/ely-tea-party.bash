# Ely Tea Party: load the usual startup files, report folder and title.
if [ -n "${ELY_BASH_LOGIN-}" ]; then
  builtin unset ELY_BASH_LOGIN
  [ -r /etc/profile ] && builtin source /etc/profile
  for _ely_file in ~/.bash_profile ~/.bash_login ~/.profile; do
    if [ -r "$_ely_file" ]; then
      builtin source "$_ely_file"
      break
    fi
  done
  builtin unset _ely_file
else
  [ -r ~/.bashrc ] && builtin source ~/.bashrc
fi
_ely_prompt() {
  builtin local ret=$? LC_ALL=C char url= i title
  for (( i = 0; i < ${#PWD}; i++ )); do
    char=${PWD:i:1}
    case $char in
      [[:cntrl:]]|%) builtin printf -v char '%%%02X' "'$char" ;;
    esac
    url+=$char
  done
  title=${PWD##*/}
  builtin printf '\e]7;file://%s\a\e]2;%s\a' "$url" "${title//[[:cntrl:]]/}"
  builtin return $ret
}
PROMPT_COMMAND="_ely_prompt${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
