#!/usr/bin/env python3
"""Graft mainline native HDMI onto a boot-tested core DTB.

Only HDMI dependencies are imported. Byte-preserving DT decoding avoids
rewriting existing regulator, SD and clock policy. Requires dtc/fdtget.
"""
import argparse
import copy
import pathlib
import subprocess


def get(blob, path, flag):
    return subprocess.check_output(['fdtget', flag, blob, path], text=True).split()


def read(blob, path='/'):
    props = {}
    for key in get(blob, path, '-p'):
        raw = subprocess.check_output(['fdtget', '-t', 'bx', blob, path, key], text=True)
        props[key] = bytes(int(x, 16) for x in raw.split())
    children = {name: read(blob, path.rstrip('/') + '/' + name)
                for name in get(blob, path, '-l')}
    return [props, children]


def at(tree, path):
    for name in path.strip('/').split('/'):
        if name:
            tree = tree[1][name]
    return tree


def walk(tree):
    yield tree
    for child in tree[1].values():
        yield from walk(child)


def cells(raw):
    assert len(raw) % 4 == 0
    return [int.from_bytes(raw[i:i+4], 'big') for i in range(0, len(raw), 4)]


def pack(values):
    return b''.join(x.to_bytes(4, 'big') for x in values)


def emit(name, tree, depth=0):
    indent = '\t' * depth
    lines = [indent + name + ' {']
    for key, value in tree[0].items():
        lines.append(indent + '\t' + key + (' = [' + value.hex(' ') + ']' if value else '') + ';')
    for key, value in tree[1].items():
        lines.extend(emit(key, value, depth + 1))
    return lines + [indent + '};']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('core')
    parser.add_argument('mainline')
    parser.add_argument('output', type=pathlib.Path)
    args = parser.parse_args()
    core, mainline = read(args.core), read(args.mainline)
    aips1 = '/soc@0/bus@30000000'
    media = '/soc@0/bus@32c00000'
    paths = [aips1 + '/gpc@303a0000', aips1 + '/pinctrl@30330000/hdmigrp',
             '/native-hdmi-connector']
    paths += [media + '/' + name for name in (
        'blk-ctrl@32fc0000', 'interrupt-controller@32fc2000',
        'display-bridge@32fc4000', 'display-controller@32fc6000',
        'hdmi@32fd8000', 'phy@32fdff00')]
    grafts = {path: copy.deepcopy(at(mainline, path)) for path in paths}
    # Vendor core hogs HDMI DDC/CEC pins. Transfer only pins requested by
    # the native TX group; retain HPD and every unrelated default setting.
    pinctrl = at(core, aips1 + '/pinctrl@30330000')
    hdmi_pins = cells(grafts[aips1 + '/pinctrl@30330000/hdmigrp'][0]['fsl,pins'])
    assert len(hdmi_pins) % 6 == 0
    hdmi_muxes = set(hdmi_pins[::6])
    hog_ids = set(cells(pinctrl[0]['pinctrl-0']))
    released = []
    for props, _ in walk(pinctrl):
        if cells(props.get('phandle', b''))[:1] not in [[h] for h in hog_ids]:
            continue
        pins = cells(props.get('fsl,pins', b''))
        assert len(pins) % 6 == 0
        kept = []
        for i in range(0, len(pins), 6):
            if pins[i] in hdmi_muxes:
                released.append(pins[i])
            else:
                kept.extend(pins[i:i+6])
        if pins:
            props['fsl,pins'] = pack(kept)
    print('Released HDMI muxes from default hog:', ', '.join(hex(p) for p in released))
    # Only HDMI GPC domains are introduced, avoiding unrelated power policy.
    domains = at(grafts[aips1 + '/gpc@303a0000'], '/pgc')[1]
    for name in list(domains):
        if name not in ('power-domain@14', 'power-domain@15'):
            del domains[name]
    # ICC paths are optional in the binding. Preserve bootloader NoC setup
    # for this video-only candidate instead of referencing an absent provider.
    blk = grafts[media + '/blk-ctrl@32fc0000'][0]
    blk.pop('interconnects', None)
    blk.pop('interconnect-names', None)
    # No audio component: remove the graph edge as well as omitting PAI.
    grafts[media + '/hdmi@32fd8000'][1]['ports'][1].pop('port@2', None)
    local_ids = {cells(n[0]['phandle'])[0] for t in grafts.values()
                 for n in walk(t) if 'phandle' in n[0]}
    mapping = {value: value + 0x1000 for value in local_ids}
    source_clk = cells(at(mainline, aips1 + '/clock-controller@30380000')[0]['phandle'])[0]
    mapping[source_clk] = cells(at(core, aips1 + '/clock-controller@30380000')[0]['phandle'])[0]
    source_gic = cells(mainline[0]['interrupt-parent'])[0]
    mapping[source_gic] = cells(core[0]['interrupt-parent'])[0]
    references = {'phandle': 1, 'linux,phandle': 1, 'interrupt-parent': 1,
                  'remote-endpoint': 1, 'pinctrl-0': 1,
                  'assigned-clocks': 2, 'assigned-clock-parents': 2}
    for tree in grafts.values():
        for props, _ in walk(tree):
            for key in references.keys() | {'clocks', 'power-domains'}:
                if key not in props:
                    continue
                values = cells(props[key])
                pos = 0
                while pos < len(values):
                    old = values[pos]
                    if old not in mapping:
                        raise ValueError(f'unresolved {key} phandle {old:#x}')
                    if key == 'clocks':
                        step = 2 if old == source_clk else 1
                    elif key == 'power-domains':
                        step = 2 if old == cells(grafts[media + '/blk-ctrl@32fc0000'][0]['phandle'])[0] else 1
                        # Block-controller phandle may already be rewritten.
                        if old in local_ids and old == cells(at(mainline, media + '/blk-ctrl@32fc0000')[0]['phandle'])[0]:
                            step = 2
                    else:
                        step = references[key]
                    values[pos] = mapping[old]
                    pos += step
                props[key] = pack(values)
    # Replace old disabled vendor placeholders at identical physical addresses.
    children = at(core, media)[1]
    for name in list(children):
        if name.split('@')[-1] in ('32fc0000', '32fc2000', '32fc4000', '32fc6000', '32fd8000', '32fdff00'):
            del children[name]
    for path, tree in grafts.items():
        parent, name = path.rsplit('/', 1)
        at(core, parent)[1][name] = tree
    # Core's 960-MiB CMA allocation failed. Place a 256-MiB pool in guest RAM,
    # above kernel/DTB load addresses and outside the DSP/non-root reservation.
    cma = at(core, '/reserved-memory/linux,cma')[0]
    cma['size'] = pack([0, 0x10000000])
    cma['alloc-ranges'] = pack([0, 0xb0000000, 0, 0x20000000])
    ids = [cells(n[0]['phandle'])[0] for n in walk(core) if 'phandle' in n[0]]
    assert len(ids) == len(set(ids)), 'duplicate phandles'
    args.output.write_text('/dts-v1/;\n' + '\n'.join(emit('/', core)) + '\n')
    print(f'Imported {len(grafts)} HDMI dependency roots; core devices preserved.')


if __name__ == '__main__':
    main()
