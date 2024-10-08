#![no_std]
#![no_main]

#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
mod tracepoint;

use crate::tracepoint::trace_event_raw_inet_sock_set_state;
use aya_ebpf::maps::PerfEventArray;
use aya_ebpf::EbpfContext;
use aya_ebpf::{macros::map, macros::tracepoint, programs::TracePointContext};
use aya_log_ebpf::info;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tcpstate_common::{Family, TcpSocketEvent, AF_INET, AF_INET6};

const IPPROTO_TCP: u16 = 6;

#[map]
pub static mut TCP_EVENTS: PerfEventArray<TcpSocketEvent> =
    PerfEventArray::with_max_entries(1024, 0);

#[tracepoint]
pub fn tcp_set_state(ctx: TracePointContext) -> i64 {
    match try_tcp_set_state(ctx) {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

fn try_tcp_set_state(ctx: TracePointContext) -> Result<i64, i64> {
    let evt_ptr = ctx.as_ptr() as *const trace_event_raw_inet_sock_set_state;
    let evt = unsafe { evt_ptr.as_ref().ok_or(1i64)? };
    if evt.protocol != IPPROTO_TCP {
        return Ok(0);
    }
    let ev = make_ev(&ctx, evt)?;
    unsafe {
        #[allow(static_mut_refs)]
        TCP_EVENTS.output(&ctx, &ev, 0);
    }

    Ok(0)
}
fn make_ev(
    ctx: &TracePointContext,
    evt: &trace_event_raw_inet_sock_set_state,
) -> Result<TcpSocketEvent, i32> {
    //let pid = helpers::bpf_get_current_pid_tgid() >> 32;
    //let comm = helpers::bpf_get_current_comm();
    let family = match evt.family {
        AF_INET => Family::IPv4,
        AF_INET6 => Family::IPv6,
        other => {
            info!(ctx, "unknown family {}", other);
            return Err(-999);
        }
    };
    let mut ip_bytes: [u8; 16] = [0; 16];

    if let Family::IPv6 = family {
        ip_bytes[..16].copy_from_slice(&evt.daddr_v6);
    }

    // The verifier can't verify this if i put it inside a `match` :'(
    // if family != Ipv6, `ip6` contains garbage, but is not returned
    let ip6 = IpAddr::V6(Ipv6Addr::from_bits(
        (ip_bytes[0] as u128) << 120
            | (ip_bytes[1] as u128) << 112
            | (ip_bytes[2] as u128) << 104
            | (ip_bytes[3] as u128) << 96
            | (ip_bytes[4] as u128) << 88
            | (ip_bytes[5] as u128) << 80
            | (ip_bytes[6] as u128) << 72
            | (ip_bytes[7] as u128) << 64
            | (ip_bytes[8] as u128) << 56
            | (ip_bytes[9] as u128) << 48
            | (ip_bytes[10] as u128) << 40
            | (ip_bytes[11] as u128) << 32
            | (ip_bytes[12] as u128) << 24
            | (ip_bytes[13] as u128) << 16
            | (ip_bytes[14] as u128) << 8
            | (ip_bytes[15] as u128) << 0,
    ));
    let ip = match family {
        Family::IPv4 => {
            ip_bytes[..4].copy_from_slice(&evt.daddr);
            let ip4 = IpAddr::V4(Ipv4Addr::new(
                ip_bytes[0],
                ip_bytes[1],
                ip_bytes[2],
                ip_bytes[3],
            ));
            ip4
        }
        Family::IPv6 => ip6,
    };

    let ev = TcpSocketEvent {
        oldstate: evt.oldstate.into(),
        newstate: evt.newstate.into(),
        sport: evt.sport,
        dport: evt.dport,
        dst: ip,
    };
    Ok(ev)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
