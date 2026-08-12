#!/bin/bash
# Line 1 (work):  [PONYTAIL] | repo@branch | S: n | U: n | A: n
# Line 2 (cost):  effort: X | tokens (pct%) | 5h pct% remaining | 7d pct% remaining | extra $used/$limit
# Colors/fields follow https://github.com/daniel3303/ClaudeCodeStatusLine
input=$(cat)

BLUE=$'\033[38;2;0;153;255m'
ORANGE=$'\033[38;2;255;176;85m'
GREEN=$'\033[38;2;0;160;0m'
CYAN=$'\033[38;2;46;149;153m'
RED=$'\033[38;2;255;85;85m'
YELLOW=$'\033[38;2;230;200;0m'
PURPLE=$'\033[38;2;167;139;250m'
WHITE=$'\033[38;2;220;220;220m'
DIM=$'\033[2m'
RESET=$'\033[0m'

usage_color() {
  local pct=$1
  if [ "$pct" -ge 90 ]; then echo "$RED"
  elif [ "$pct" -ge 70 ]; then echo "$ORANGE"
  elif [ "$pct" -ge 50 ]; then echo "$YELLOW"
  else echo "$GREEN"
  fi
}

format_tokens() {
  local num=$1
  if [ "$num" -ge 1000000 ]; then
    awk "BEGIN {v=sprintf(\"%.1f\",$num/1000000)+0; if(v==int(v)) printf \"%dm\",v; else printf \"%.1fm\",v}"
  elif [ "$num" -ge 1000 ]; then
    awk "BEGIN {printf \"%.0fk\", $num/1000}"
  else
    printf "%d" "$num"
  fi
}

# ===== Line 1: work (repo@branch, git file counts) =====
cwd=$(echo "$input" | jq -r '.workspace.current_dir // .cwd')

toplevel=""
if git -C "$cwd" --no-optional-locks rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  toplevel=$(git -C "$cwd" --no-optional-locks rev-parse --show-toplevel 2>/dev/null)
fi
repo=$(basename "${toplevel:-$cwd}")

work=()

ponytail_hook=$(ls "$HOME"/.claude/plugins/cache/ponytail/ponytail/*/hooks/ponytail-statusline.sh 2>/dev/null | sort -V | tail -1)
if [ -n "$ponytail_hook" ]; then
  badge=$(bash "$ponytail_hook")
  [ -n "$badge" ] && work+=("$badge")
fi

if [ -n "$toplevel" ]; then
  branch=$(git -C "$cwd" --no-optional-locks branch --show-current 2>/dev/null)
  staged=$(git -C "$cwd" --no-optional-locks diff --cached --name-only 2>/dev/null | wc -l | tr -d ' ')
  unstaged=$(git -C "$cwd" --no-optional-locks diff --name-only 2>/dev/null | wc -l | tr -d ' ')
  added=$(git -C "$cwd" --no-optional-locks ls-files --others --exclude-standard 2>/dev/null | wc -l | tr -d ' ')

  work+=("${CYAN}${repo}${RESET}${DIM}@${RESET}${GREEN}${branch}${RESET}")
  work+=("S: ${YELLOW}${staged}${RESET} | U: ${YELLOW}${unstaged}${RESET} | A: ${YELLOW}${added}${RESET}")
else
  work+=("${CYAN}${repo}${RESET}")
fi

line1=$(printf '%s | ' "${work[@]}")
line1="${line1% | }"

# ===== Line 2: cost (effort, tokens, 5h/7d rate limits, extra usage) =====
cost=()

effort_level=$(echo "$input" | jq -r '.effort.level // empty')
[ -z "$effort_level" ] && effort_level="medium"
case "$effort_level" in
  low)    effort_disp="${DIM}${effort_level}${RESET}" ;;
  medium) effort_disp="${ORANGE}med${RESET}" ;;
  high)   effort_disp="${GREEN}${effort_level}${RESET}" ;;
  xhigh)  effort_disp="${PURPLE}${effort_level}${RESET}" ;;
  max)    effort_disp="${RED}${effort_level}${RESET}" ;;
  *)      effort_disp="${GREEN}${effort_level}${RESET}" ;;
esac
cost+=("effort: ${effort_disp}")

input_tokens=$(echo "$input" | jq -r '.context_window.current_usage.input_tokens // 0')
cache_create=$(echo "$input" | jq -r '.context_window.current_usage.cache_creation_input_tokens // 0')
cache_read=$(echo "$input" | jq -r '.context_window.current_usage.cache_read_input_tokens // 0')
current=$(( input_tokens + cache_create + cache_read ))
size=$(echo "$input" | jq -r '.context_window.context_window_size // 200000')
[ "$size" -eq 0 ] 2>/dev/null && size=200000
pct_used=$(( size > 0 ? current * 100 / size : 0 ))
cost+=("${ORANGE}$(format_tokens "$current")/$(format_tokens "$size")${RESET} ${DIM}(${RESET}${GREEN}${pct_used}%${RESET}${DIM})${RESET}")

five_pct=$(echo "$input" | jq -r '.rate_limits.five_hour.used_percentage // empty')
five_reset=$(echo "$input" | jq -r '.rate_limits.five_hour.resets_at // empty')
if [ -n "$five_pct" ]; then
  five_pct=$(printf '%.0f' "$five_pct")
  seg="${WHITE}5h${RESET} $(usage_color "$five_pct")${five_pct}%${RESET}"
  if [ -n "$five_reset" ] && [ "$five_reset" != "null" ]; then
    diff=$(( ${five_reset%.*} - $(date +%s) ))
    [ "$diff" -lt 0 ] && diff=0
    t="$((diff/3600))h$(((diff%3600)/60))m"
    seg="${seg} ${DIM}${t}${RESET}"
  fi
  cost+=("$seg")
else
  cost+=("${WHITE}5h${RESET} ${DIM}-${RESET}")
fi

seven_pct=$(echo "$input" | jq -r '.rate_limits.seven_day.used_percentage // empty')
seven_reset=$(echo "$input" | jq -r '.rate_limits.seven_day.resets_at // empty')
if [ -n "$seven_pct" ]; then
  seven_pct=$(printf '%.0f' "$seven_pct")
  seg="${WHITE}7d${RESET} $(usage_color "$seven_pct")${seven_pct}%${RESET}"
  if [ -n "$seven_reset" ] && [ "$seven_reset" != "null" ]; then
    diff=$(( ${seven_reset%.*} - $(date +%s) ))
    [ "$diff" -lt 0 ] && diff=0
    t="$((diff/86400))d$(((diff%86400)/3600))h"
    seg="${seg} ${DIM}${t}${RESET}"
  fi
  cost+=("$seg")
else
  cost+=("${WHITE}7d${RESET} ${DIM}-${RESET}")
fi

# Extra usage isn't in stdin JSON — needs the OAuth usage API, cached 60s.
get_oauth_token() {
  [ -n "$CLAUDE_CODE_OAUTH_TOKEN" ] && { printf '%s' "$CLAUDE_CODE_OAUTH_TOKEN"; return; }
  local creds="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/.credentials.json"
  [ -f "$creds" ] && jq -r '.claudeAiOauth.accessToken // empty' "$creds" 2>/dev/null
}

cache_file="/tmp/claude-statusline-extra-cache.json"
mkdir -p /tmp
extra_data=""
if [ -f "$cache_file" ]; then
  age=$(( $(date +%s) - $(stat -c %Y "$cache_file" 2>/dev/null || echo 0) ))
  [ "$age" -lt 60 ] && extra_data=$(cat "$cache_file")
fi
if [ -z "$extra_data" ]; then
  token=$(get_oauth_token)
  if [ -n "$token" ] && [ "$token" != "null" ]; then
    resp=$(curl -s --max-time 5 \
      -H "Authorization: Bearer $token" \
      -H "anthropic-beta: oauth-2025-04-20" \
      -H "Content-Type: application/json" \
      "https://api.anthropic.com/api/oauth/usage" 2>/dev/null)
    if echo "$resp" | jq -e '.extra_usage' >/dev/null 2>&1; then
      extra_data="$resp"
      printf '%s' "$resp" > "$cache_file"
    fi
  fi
fi

if [ -n "$extra_data" ] && [ "$(echo "$extra_data" | jq -r '.extra_usage.is_enabled // false')" = "true" ]; then
  pct=$(echo "$extra_data" | jq -r '.extra_usage.utilization // 0' | awk '{printf "%.0f", $1}')
  used=$(echo "$extra_data" | jq -r '.extra_usage.used_credits // 0' | awk '{printf "%.2f", $1/100}')
  limit=$(echo "$extra_data" | jq -r '.extra_usage.monthly_limit // 0' | awk '{printf "%.2f", $1/100}')
  cost+=("${WHITE}extra${RESET} $(usage_color "$pct")\$${used}/\$${limit}${RESET}")
fi

line2=$(printf '%s | ' "${cost[@]}")
line2="${line2% | }"

printf '%s\n%s' "$line1" "$line2"
