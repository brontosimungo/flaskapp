#!/usr/bin/env bash
set -Eeuo pipefail

# BW Mining – helper launcher for nockpool-miner
# Features:
# - subcommands: start | stop | restart | status | logs | help
# - options: --account-token, --max-threads, --jam, --lib-dir, --miner-bin,
#            --log-file, --pidfile, --daemon, --extra-args
# - environment overrides: ACCESS_TOKEN, MAX_THREADS, MINER_JAM_PATH, LIB_DIR,
#                          MINER_BIN, LOG_FILE, PIDFILE, EXTRA_ARGS

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
DEFAULT_MINER_BIN="${SCRIPT_DIR}/nockpool-miner"
DEFAULT_JAM_PATH="${SCRIPT_DIR}/miner.jam"
DEFAULT_LIB_DIR="${SCRIPT_DIR}"
DEFAULT_LOG_FILE="${SCRIPT_DIR}/nockpool.log"
DEFAULT_PIDFILE="${SCRIPT_DIR}/.nockpool-miner.pid"

MINER_BIN="${MINER_BIN:-$DEFAULT_MINER_BIN}"
JAM_PATH="${MINER_JAM_PATH:-$DEFAULT_JAM_PATH}"
LIB_DIR="${LIB_DIR:-$DEFAULT_LIB_DIR}"
LOG_FILE="${LOG_FILE:-$DEFAULT_LOG_FILE}"
PIDFILE="${PIDFILE:-$DEFAULT_PIDFILE}"
ACCOUNT_TOKEN="${ACCESS_TOKEN:-}" # required
THREADS="${MAX_THREADS:-$(nproc || echo 8)}"
EXTRA_ARGS="${EXTRA_ARGS:-}"
DAEMON=0

usage() {
	cat <<EOF
Usage: $0 <command> [options]

Commands:
  start       Start the miner (foreground by default)
  stop        Stop the running miner
  restart     Stop then start
  status      Print running status
  logs        Tail the log file
  help        Show this help

Options:
  --account-token TOKEN    Pool account token (required on first start)
  --max-threads N          Max CPU threads (default: ${THREADS})
  --jam PATH               Path to miner.jam (default: ${JAM_PATH})
  --lib-dir DIR            Directory of libzkvm_jetpack.so (default: ${LIB_DIR})
  --miner-bin PATH         Path to nockpool-miner (default: ${MINER_BIN})
  --log-file PATH          Log file (default: ${LOG_FILE})
  --pidfile PATH           PID file (default: ${PIDFILE})
  --daemon                 Run in background (nohup)
  --extra-args "..."       Extra args passed to miner as-is

Environment overrides:
  ACCESS_TOKEN, MAX_THREADS, MINER_JAM_PATH, LIB_DIR, MINER_BIN, LOG_FILE, PIDFILE, EXTRA_ARGS

Examples:
  $0 start --account-token nockacct_xxx --max-threads 16
  $0 start --daemon --account-token nockacct_xxx --max-threads 32
  $0 status | $0 logs | $0 stop
EOF
}

log() { echo "[nockpool-run] $*"; }
die() {
	echo "[nockpool-run][ERROR] $*" >&2
	exit 1
}

is_running() {
	if [[ -f "$PIDFILE" ]]; then
		local pid
		pid=$(<"$PIDFILE") || true
		if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
			return 0
		fi
	fi
	return 1
}

cmd_status() {
	if is_running; then
		local pid
		pid=$(<"$PIDFILE")
		log "running (pid=$pid)"
		exit 0
	else
		log "stopped"
		exit 3
	fi
}

cmd_logs() {
	[[ -f "$LOG_FILE" ]] || die "log file not found: $LOG_FILE"
	exec tail -n 100 -F "$LOG_FILE"
}

parse_opts() {
	# parse long options manually to avoid non-portable getopt behavior
	while [[ $# -gt 0 ]]; do
		case "$1" in
		--account-token)
			ACCOUNT_TOKEN="$2"
			shift 2
			;;
		--max-threads)
			THREADS="$2"
			shift 2
			;;
		--jam)
			JAM_PATH="$2"
			shift 2
			;;
		--lib-dir)
			LIB_DIR="$2"
			shift 2
			;;
		--miner-bin)
			MINER_BIN="$2"
			shift 2
			;;
		--log-file)
			LOG_FILE="$2"
			shift 2
			;;
		--pidfile)
			PIDFILE="$2"
			shift 2
			;;
		--daemon)
			DAEMON=1
			shift
			;;
		--extra-args)
			EXTRA_ARGS="$2"
			shift 2
			;;
		--help | -h)
			usage
			exit 0
			;;
		--)
			shift
			break
			;;
		*) die "unknown option: $1" ;;
		esac
	done
}

ensure_requirements() {
	[[ -x "$MINER_BIN" ]] || die "miner binary not found or not executable: $MINER_BIN"
	[[ -f "$JAM_PATH" ]] || die "miner.jam not found: $JAM_PATH"
	[[ -f "$LIB_DIR/libzkvm_jetpack.so" ]] || die "lib not found: $LIB_DIR/libzkvm_jetpack.so"
	if [[ -z "${ACCOUNT_TOKEN:-}" ]]; then
		die "--account-token is required (or set ACCESS_TOKEN env)"
	fi
}

cmd_start() {
	parse_opts "$@"
	ensure_requirements

	export MINER_JAM_PATH="$JAM_PATH"
	export LD_LIBRARY_PATH="$LIB_DIR:${LD_LIBRARY_PATH:-}"

	if is_running; then
		local pid
		pid=$(<"$PIDFILE")
		die "already running (pid=$pid); use '$0 restart' or '$0 stop' first"
	fi

	local args=("--max-threads" "$THREADS" "--account-token" "$ACCOUNT_TOKEN")
	if [[ -n "$EXTRA_ARGS" ]]; then
		# shellcheck disable=SC2206
		extra=($EXTRA_ARGS)
		args+=("${extra[@]}")
	fi

	if [[ "$DAEMON" -eq 1 ]]; then
		log "starting in background → log: $LOG_FILE, pid: $PIDFILE"
		nohup "$MINER_BIN" "${args[@]}" >>"$LOG_FILE" 2>&1 </dev/null &
		echo $! >"$PIDFILE"
		sleep 0.2
		cmd_status
	else
		log "starting in foreground"
		log "MINER_JAM_PATH=$MINER_JAM_PATH | LIB_DIR=$LIB_DIR | threads=$THREADS"
		exec "$MINER_BIN" "${args[@]}" | tee -a "$LOG_FILE"
	fi
}

cmd_stop() {
	if ! is_running; then
		log "already stopped"
		exit 0
	fi
	local pid
	pid=$(<"$PIDFILE")
	log "stopping pid=$pid"
	kill "$pid" 2>/dev/null || true
	for i in {1..50}; do
		if ! kill -0 "$pid" 2>/dev/null; then
			rm -f "$PIDFILE"
			log "stopped"
			exit 0
		fi
		sleep 0.1
	done
	log "force kill pid=$pid"
	kill -KILL "$pid" 2>/dev/null || true
	rm -f "$PIDFILE"
}

cmd_restart() {
	cmd_stop || true
	cmd_start "$@"
}

main() {
	local cmd="${1:-help}"
	shift || true
	case "$cmd" in
	start) cmd_start "$@" ;;
	stop) cmd_stop ;;
	restart) cmd_restart "$@" ;;
	status) cmd_status ;;
	logs) cmd_logs ;;
	help | -h | --help) usage ;;
	*)
		echo "Unknown command: $cmd"
		usage
		exit 2
		;;
	esac
}

main "$@"
