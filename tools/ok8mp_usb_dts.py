#!/usr/bin/env python3
"""Add the board's mainline USB host/HSIO dependencies to tested HDMI R2."""
import argparse
import copy
from pathlib import Path
from ok8mp_hdmi_dts import read, at, walk, cells, pack, emit


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('base')
    p.add_argument('mainline')
    p.add_argument('output', type=Path)
    a = p.parse_args()
    base, src = read(a.base), read(a.mainline)
    bus = '/soc@0/bus@30000000'
    gpc = bus + '/gpc@303a0000/pgc'
    paths = [gpc + '/power-domain@' + n for n in ('1', '2', '3', '17')]
    paths += [bus + '/pinctrl@30330000/usb1grp',
              '/soc@0/bus@32c00000/blk-ctrl@32f10000']
    paths += ['/soc@0/' + n for n in ('usb-phy@381f0040', 'usb-phy@382f0040',
                                     'usb@32f10100', 'usb@32f10108')]
    grafts = {path: copy.deepcopy(at(src, path)) for path in paths}
    hsio = grafts['/soc@0/bus@32c00000/blk-ctrl@32f10000'][0]
    # Optional ICC policy is left at firmware defaults, as in HDMI R2.
    hsio.pop('interconnects', None)
    hsio.pop('interconnect-names', None)
    providers = {cells(n[0]['phandle'])[0]: n[0] for n in walk(src) if 'phandle' in n[0]}
    mapping = {cells(n[0]['phandle'])[0]: cells(n[0]['phandle'])[0] + 0x2000
               for t in grafts.values() for n in walk(t) if 'phandle' in n[0]}
    clk = bus + '/clock-controller@30380000'
    mapping[cells(at(src, clk)[0]['phandle'])[0]] = cells(at(base, clk)[0]['phandle'])[0]
    mapping[cells(src[0]['interrupt-parent'])[0]] = cells(base[0]['interrupt-parent'])[0]
    refs = {'clocks': '#clock-cells', 'assigned-clocks': '#clock-cells',
            'assigned-clock-parents': '#clock-cells', 'power-domains': '#power-domain-cells',
            'phys': '#phy-cells', 'pinctrl-0': None, 'interrupt-parent': None,
            'phandle': None, 'linux,phandle': None}
    for tree in grafts.values():
        for props, _ in walk(tree):
            for key, count in refs.items():
                if key not in props:
                    continue
                vals = cells(props[key])
                i = 0
                while i < len(vals):
                    old = vals[i]
                    step = 1 + (cells(providers[old][count])[0] if count else 0)
                    vals[i] = mapping[old]  # Fail closed on missing dependencies.
                    i += step
                assert i == len(vals), key
                props[key] = pack(vals)
    for path, tree in grafts.items():
        parent, name = path.rsplit('/', 1)
        at(base, parent)[1][name] = tree
    ids = [cells(n[0]['phandle'])[0] for n in walk(base) if 'phandle' in n[0]]
    assert len(ids) == len(set(ids)), 'duplicate phandles'
    a.output.write_text('/dts-v1/;\n' + '\n'.join(emit('/', base)) + '\n')
    print('Imported USB/HSIO dependencies; mainline port modes preserved.')


if __name__ == '__main__':
    main()
