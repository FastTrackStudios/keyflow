if [ -n "$PS1" ] && [ -e $HOME/.bashrc ]; then
    source $HOME/.bashrc;
fi

shopt -u expand_aliases
PATH=${PATH:-}
nix_saved_PATH="$PATH"
XDG_DATA_DIRS=${XDG_DATA_DIRS:-}
nix_saved_XDG_DATA_DIRS="$XDG_DATA_DIRS"
CMAKE_INCLUDE_PATH='/nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev/include:/nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/include:/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/include:/nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev/include:/nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev/include:/nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev/include:/nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42/include:/nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev/include:/nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/include:/nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/include:/nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev/include:/nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev/include:/nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/include:/nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/include:/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/include:/nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/include:/nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev/include:/nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev/include:/nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev/include:/nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/include:/nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev/include:/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/include:/nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/include:/nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/include:/nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev/include:/nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/include:/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/include:/nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev/include:/nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/include:/nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev/include:/nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev/include:/nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev/include:/nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev/include:/nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev/include:/nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/include:/nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev/include:/nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev/include:/nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev/include:/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/include:/nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev/include:/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/include:/nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev/include:/nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/include:/nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev/include:/nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev/include:/nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev/include:/nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev/include:/nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev/include:/nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev/include:/nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev/include:/nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev/include:/nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/include:/nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/include:/nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev/include:/nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev/include:/nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev/include:/nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev/include:/nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/include:/nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev/include:/nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/include:/nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev/include:/nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/include:/nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/include:/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/include:/nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev/include:/nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/include:/nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev/include:/nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev/include:/nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev/include:/nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev/include:/nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev/include:/nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev/include:/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/include:/nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/include:/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/include'
export CMAKE_INCLUDE_PATH
NIX_CC_WRAPPER_TARGET_BUILD_x86_64_unknown_linux_gnu='1'
export NIX_CC_WRAPPER_TARGET_BUILD_x86_64_unknown_linux_gnu
builder='/nix/store/9ipfvwnqp1q8ijnmi5sxvlx9r8w34lw3-bash-5.3p15/bin/bash'
export builder
PYTHONHASHSEED='0'
export PYTHONHASHSEED
outputBin='out'
outputDev='out'
SIZE_FOR_BUILD='size'
export SIZE_FOR_BUILD
checkPhase='justCheckPhase'
DEVENV_STATE='/run/media/Development/wt/keyflow-control/.devenv/state'
export DEVENV_STATE
NIX_CFLAGS_COMPILE_FOR_BUILD=' -isystem /nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev/include -isystem /nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/include -isystem /nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/include -isystem /nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev/include -isystem /nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev/include -isystem /nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev/include -isystem /nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42/include -isystem /nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev/include -isystem /nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/include -isystem /nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/include -isystem /nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev/include -isystem /nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev/include -isystem /nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/include -isystem /nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/include -isystem /nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/include -isystem /nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/include -isystem /nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev/include -isystem /nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev/include -isystem /nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev/include -isystem /nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/include -isystem /nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev/include -isystem /nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/include -isystem /nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/include -isystem /nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/include -isystem /nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev/include -isystem /nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/include -isystem /nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/include -isystem /nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev/include -isystem /nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/include -isystem /nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev/include -isystem /nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev/include -isystem /nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev/include -isystem /nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev/include -isystem /nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev/include -isystem /nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/include -isystem /nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev/include -isystem /nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev/include -isystem /nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev/include -isystem /nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/include -isystem /nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev/include -isystem /nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/include -isystem /nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev/include -isystem /nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/include -isystem /nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev/include -isystem /nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev/include -isystem /nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev/include -isystem /nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev/include -isystem /nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev/include -isystem /nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev/include -isystem /nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev/include -isystem /nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev/include -isystem /nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/include -isystem /nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/include -isystem /nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev/include -isystem /nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev/include -isystem /nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev/include -isystem /nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev/include -isystem /nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/include -isystem /nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev/include -isystem /nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/include -isystem /nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev/include -isystem /nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/include -isystem /nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/include -isystem /nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/include -isystem /nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev/include -isystem /nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/include -isystem /nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev/include -isystem /nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev/include -isystem /nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev/include -isystem /nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev/include -isystem /nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev/include -isystem /nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev/include -isystem /nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/include -isystem /nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/include -isystem /nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/include -isystem /nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev/include -isystem /nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/include -isystem /nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/include -isystem /nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev/include -isystem /nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev/include -isystem /nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev/include -isystem /nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42/include -isystem /nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev/include -isystem /nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/include -isystem /nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/include -isystem /nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev/include -isystem /nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev/include -isystem /nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/include -isystem /nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/include -isystem /nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/include -isystem /nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/include -isystem /nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev/include -isystem /nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev/include -isystem /nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev/include -isystem /nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/include -isystem /nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev/include -isystem /nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/include -isystem /nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/include -isystem /nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/include -isystem /nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev/include -isystem /nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/include -isystem /nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/include -isystem /nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev/include -isystem /nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/include -isystem /nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev/include -isystem /nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev/include -isystem /nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev/include -isystem /nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev/include -isystem /nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev/include -isystem /nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/include -isystem /nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev/include -isystem /nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev/include -isystem /nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev/include -isystem /nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/include -isystem /nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev/include -isystem /nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/include -isystem /nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev/include -isystem /nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/include -isystem /nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev/include -isystem /nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev/include -isystem /nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev/include -isystem /nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev/include -isystem /nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev/include -isystem /nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev/include -isystem /nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev/include -isystem /nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev/include -isystem /nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/include -isystem /nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/include -isystem /nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev/include -isystem /nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev/include -isystem /nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev/include -isystem /nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev/include -isystem /nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/include -isystem /nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev/include -isystem /nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/include -isystem /nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev/include -isystem /nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/include -isystem /nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/include -isystem /nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/include -isystem /nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev/include -isystem /nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/include -isystem /nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev/include -isystem /nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev/include -isystem /nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev/include -isystem /nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev/include -isystem /nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev/include -isystem /nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev/include -isystem /nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/include -isystem /nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/include -isystem /nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/include -isystem /nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev/include -isystem /nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/include -isystem /nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/include -isystem /nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev/include -isystem /nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev/include -isystem /nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev/include -isystem /nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42/include -isystem /nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev/include -isystem /nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/include -isystem /nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/include -isystem /nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev/include -isystem /nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev/include -isystem /nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/include -isystem /nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/include -isystem /nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/include -isystem /nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/include -isystem /nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev/include -isystem /nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev/include -isystem /nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev/include -isystem /nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/include -isystem /nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev/include -isystem /nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/include -isystem /nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/include -isystem /nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/include -isystem /nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev/include -isystem /nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/include -isystem /nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/include -isystem /nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev/include -isystem /nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/include -isystem /nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev/include -isystem /nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev/include -isystem /nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev/include -isystem /nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev/include -isystem /nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev/include -isystem /nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/include -isystem /nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev/include -isystem /nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev/include -isystem /nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev/include -isystem /nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/include -isystem /nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev/include -isystem /nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/include -isystem /nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev/include -isystem /nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/include -isystem /nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev/include -isystem /nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev/include -isystem /nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev/include -isystem /nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev/include -isystem /nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev/include -isystem /nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev/include -isystem /nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev/include -isystem /nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev/include -isystem /nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/include -isystem /nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/include -isystem /nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev/include -isystem /nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev/include -isystem /nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev/include -isystem /nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev/include -isystem /nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/include -isystem /nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev/include -isystem /nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/include -isystem /nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev/include -isystem /nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/include -isystem /nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/include -isystem /nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/include -isystem /nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev/include -isystem /nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/include -isystem /nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev/include -isystem /nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev/include -isystem /nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev/include -isystem /nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev/include -isystem /nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev/include -isystem /nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev/include -isystem /nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/include -isystem /nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/include -isystem /nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/include'
export NIX_CFLAGS_COMPILE_FOR_BUILD
NIX_STORE='/nix/store'
export NIX_STORE
OBJCOPY_FOR_BUILD='objcopy'
export OBJCOPY_FOR_BUILD
NIX_LDFLAGS_FOR_BUILD=' -L/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/lib -L/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8/lib -L/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/lib -L/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0/lib -L/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib -L/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2/lib -L/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8/lib -L/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib/lib -L/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58/lib -L/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/lib -L/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3/lib -L/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/lib -L/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib -L/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/lib -L/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/lib -L/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22/lib -L/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/lib -L/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0/lib -L/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/lib -L/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/lib -L/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib/lib -L/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib -L/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/lib -L/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12/lib -L/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7/lib -L/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/lib -L/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12/lib -L/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0/lib -L/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4/lib -L/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16/lib -L/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1/lib -L/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/lib -L/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/lib -L/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3/lib -L/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/lib -L/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0/lib -L/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2/lib -L/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib -L/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2/lib -L/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6/lib -L/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2/lib -L/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7/lib -L/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3/lib -L/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7/lib -L/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3/lib -L/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5/lib -L/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/lib -L/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1/lib -L/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9/lib -L/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1/lib -L/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0/lib -L/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0/lib -L/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6/lib -L/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0/lib -L/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib/lib -L/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/lib -L/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/lib -L/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/lib -L/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib -L/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/lib -L/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0/lib -L/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack/lib -L/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10/lib -L/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3/lib -L/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3/lib -L/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4/lib -L/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9/lib -L/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3/lib -L/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/lib -L/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2/lib -L/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/lib -L/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/lib -L/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8/lib -L/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/lib -L/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0/lib -L/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib -L/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2/lib -L/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8/lib -L/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib/lib -L/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58/lib -L/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/lib -L/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3/lib -L/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/lib -L/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib -L/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/lib -L/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/lib -L/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22/lib -L/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/lib -L/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0/lib -L/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/lib -L/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/lib -L/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib/lib -L/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib -L/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/lib -L/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12/lib -L/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7/lib -L/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/lib -L/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12/lib -L/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0/lib -L/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4/lib -L/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16/lib -L/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1/lib -L/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/lib -L/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/lib -L/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3/lib -L/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/lib -L/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0/lib -L/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2/lib -L/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib -L/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2/lib -L/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6/lib -L/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2/lib -L/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7/lib -L/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3/lib -L/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7/lib -L/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3/lib -L/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5/lib -L/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/lib -L/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1/lib -L/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9/lib -L/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1/lib -L/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0/lib -L/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0/lib -L/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6/lib -L/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0/lib -L/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib/lib -L/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/lib -L/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/lib -L/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/lib -L/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib -L/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/lib -L/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0/lib -L/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack/lib -L/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10/lib -L/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3/lib -L/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3/lib -L/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4/lib -L/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9/lib -L/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3/lib -L/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/lib -L/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2/lib -L/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/lib -L/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/lib -L/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8/lib -L/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/lib -L/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0/lib -L/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib -L/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2/lib -L/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8/lib -L/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib/lib -L/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58/lib -L/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/lib -L/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3/lib -L/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/lib -L/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib -L/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/lib -L/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/lib -L/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22/lib -L/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/lib -L/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0/lib -L/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/lib -L/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/lib -L/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib/lib -L/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib -L/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/lib -L/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12/lib -L/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7/lib -L/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/lib -L/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12/lib -L/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0/lib -L/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4/lib -L/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16/lib -L/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1/lib -L/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/lib -L/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/lib -L/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3/lib -L/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/lib -L/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0/lib -L/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2/lib -L/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib -L/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2/lib -L/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6/lib -L/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2/lib -L/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7/lib -L/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3/lib -L/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7/lib -L/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3/lib -L/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5/lib -L/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/lib -L/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1/lib -L/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9/lib -L/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1/lib -L/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0/lib -L/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0/lib -L/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6/lib -L/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0/lib -L/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib/lib -L/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/lib -L/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/lib -L/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/lib -L/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib -L/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/lib -L/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0/lib -L/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack/lib -L/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10/lib -L/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3/lib -L/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3/lib -L/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4/lib -L/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9/lib -L/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3/lib -L/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/lib -L/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2/lib -L/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/lib'
export NIX_LDFLAGS_FOR_BUILD
defaultBuildInputs=''
depsTargetTarget=''
export depsTargetTarget
dontAddDisableDepTrack='1'
export dontAddDisableDepTrack
LD_LIBRARY_PATH='/nix/store/0vqb1mcas5j8dv6bhbrshinlgsg6bvgi-gcc-15.3.0-lib/lib:/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/lib:/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/lib:/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib:/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/lib:/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22/lib:/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/lib:/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0/lib:/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0/lib:/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/lib:/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/lib:/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib:/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1/lib:/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4/lib:/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib:/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/lib:/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0/lib:/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/lib:/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0/lib:/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/lib:/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/lib:/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib'
export LD_LIBRARY_PATH
depsBuildBuildPropagated=''
export depsBuildBuildPropagated
OBJCOPY='objcopy'
export OBJCOPY
declare -a envBuildHostHooks=('ccWrapper_addCVars' 'bintoolsWrapper_addLDVars' 'addPythonPath' 'sysconfigdataHook' 'gettextDataDirsHook' )
name='devenv-shell-env'
export name
__structuredAttrs=''
export __structuredAttrs
depsBuildTarget=''
export depsBuildTarget
BINDGEN_EXTRA_CLANG_ARGS='-B/nix/store/4yxdxw3kppr2pbic95p4mri520rkdag2-gcc-15.3.0/lib/gcc/x86_64-unknown-linux-gnu/15.3.0 --gcc-toolchain=/nix/store/4yxdxw3kppr2pbic95p4mri520rkdag2-gcc-15.3.0 -B/nix/store/yc2a9854a2y2c8kci88piblc847iq1l4-clang-21.1.8-lib/lib -nostdlibinc -resource-dir=/nix/store/w021fbcg4z6vxihnp6gb6vijyifl051f-clang-wrapper-21.1.8/resource-root -idirafter /nix/store/qwi863qp79xkdm2yw3xzjh73dpg9qh67-glibc-2.42-84-dev/include -fmacro-prefix-map=/nix/store/qwi863qp79xkdm2yw3xzjh73dpg9qh67-glibc-2.42-84-dev/include=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-glibc-2.42-84-dev/include  -cxx-isystem /nix/store/4yxdxw3kppr2pbic95p4mri520rkdag2-gcc-15.3.0/include/c++/15.3.0 -fmacro-prefix-map=/nix/store/4yxdxw3kppr2pbic95p4mri520rkdag2-gcc-15.3.0/include/c++/15.3.0=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-gcc-15.3.0/include/c++/15.3.0 -cxx-isystem /nix/store/4yxdxw3kppr2pbic95p4mri520rkdag2-gcc-15.3.0/include/c++/15.3.0/x86_64-unknown-linux-gnu -fmacro-prefix-map=/nix/store/4yxdxw3kppr2pbic95p4mri520rkdag2-gcc-15.3.0/include/c++/15.3.0/x86_64-unknown-linux-gnu=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-gcc-15.3.0/include/c++/15.3.0/x86_64-unknown-linux-gnu   -frandom-seed=w5zw86r6j0 -isystem /nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev/include -isystem /nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/include -isystem /nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/include -isystem /nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev/include -isystem /nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev/include -isystem /nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev/include -isystem /nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42/include -isystem /nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev/include -isystem /nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/include -isystem /nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/include -isystem /nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev/include -isystem /nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev/include -isystem /nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/include -isystem /nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/include -isystem /nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/include -isystem /nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/include -isystem /nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev/include -isystem /nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev/include -isystem /nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev/include -isystem /nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/include -isystem /nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev/include -isystem /nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/include -isystem /nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/include -isystem /nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/include -isystem /nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev/include -isystem /nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/include -isystem /nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/include -isystem /nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev/include -isystem /nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/include -isystem /nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev/include -isystem /nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev/include -isystem /nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev/include -isystem /nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev/include -isystem /nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev/include -isystem /nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/include -isystem /nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev/include -isystem /nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev/include -isystem /nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev/include -isystem /nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/include -isystem /nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev/include -isystem /nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/include -isystem /nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev/include -isystem /nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/include -isystem /nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev/include -isystem /nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev/include -isystem /nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev/include -isystem /nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev/include -isystem /nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev/include -isystem /nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev/include -isystem /nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev/include -isystem /nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev/include -isystem /nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/include -isystem /nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/include -isystem /nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev/include -isystem /nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev/include -isystem /nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev/include -isystem /nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev/include -isystem /nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/include -isystem /nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev/include -isystem /nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/include -isystem /nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev/include -isystem /nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/include -isystem /nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/include -isystem /nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/include -isystem /nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev/include -isystem /nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/include -isystem /nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev/include -isystem /nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev/include -isystem /nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev/include -isystem /nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev/include -isystem /nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev/include -isystem /nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev/include -isystem /nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/include -isystem /nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/include -isystem /nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/include -isystem /nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev/include -isystem /nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/include -isystem /nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/include -isystem /nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev/include -isystem /nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev/include -isystem /nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev/include -isystem /nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42/include -isystem /nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev/include -isystem /nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/include -isystem /nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/include -isystem /nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev/include -isystem /nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev/include -isystem /nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/include -isystem /nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/include -isystem /nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/include -isystem /nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/include -isystem /nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev/include -isystem /nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev/include -isystem /nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev/include -isystem /nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/include -isystem /nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev/include -isystem /nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/include -isystem /nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/include -isystem /nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/include -isystem /nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev/include -isystem /nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/include -isystem /nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/include -isystem /nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev/include -isystem /nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/include -isystem /nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev/include -isystem /nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev/include -isystem /nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev/include -isystem /nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev/include -isystem /nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev/include -isystem /nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/include -isystem /nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev/include -isystem /nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev/include -isystem /nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev/include -isystem /nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/include -isystem /nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev/include -isystem /nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/include -isystem /nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev/include -isystem /nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/include -isystem /nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev/include -isystem /nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev/include -isystem /nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev/include -isystem /nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev/include -isystem /nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev/include -isystem /nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev/include -isystem /nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev/include -isystem /nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev/include -isystem /nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/include -isystem /nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/include -isystem /nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev/include -isystem /nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev/include -isystem /nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev/include -isystem /nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev/include -isystem /nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/include -isystem /nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev/include -isystem /nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/include -isystem /nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev/include -isystem /nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/include -isystem /nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/include -isystem /nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/include -isystem /nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev/include -isystem /nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/include -isystem /nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev/include -isystem /nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev/include -isystem /nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev/include -isystem /nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev/include -isystem /nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev/include -isystem /nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev/include -isystem /nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/include -isystem /nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/include -isystem /nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/include'
export BINDGEN_EXTRA_CLANG_ARGS
outputs='out'
export outputs
declare -a propagatedBuildDepFiles=('propagated-build-build-deps' 'propagated-native-build-inputs' 'propagated-build-target-deps' )
RANLIB_FOR_BUILD='ranlib'
export RANLIB_FOR_BUILD
AR='ar'
export AR
OPENSSL_DIR='/nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev'
export OPENSSL_DIR
configureFlags=''
export configureFlags
ORT_DYLIB_PATH='/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/lib/libonnxruntime.so'
export ORT_DYLIB_PATH
propagatedBuildInputs=''
export propagatedBuildInputs
OSTYPE='linux-gnu'
depsHostHost=''
export depsHostHost
PKG_CONFIG='pkg-config'
export PKG_CONFIG
hardeningDisable=''
export hardeningDisable
outputDevman='out'
declare -a envTargetTargetHooks=()
declare -a preConfigureHooks=('_multioutConfig' )
initialPath='/nix/store/3qgy8q2j64v2m9jy3a5jmssacbblhd4r-coreutils-9.11 /nix/store/i4kjidyvl8ciwbcwm75g75sriq3knav0-findutils-4.11.0 /nix/store/xw09m9cnc9ayb2j53y2rj8jhm1h7kkxa-diffutils-3.12 /nix/store/4nylac9gazdpsid79qyk0jpqm312b2jx-gnused-4.10 /nix/store/yhr4ps9rdmb55pgqaq246vlh7xfac1dx-gnugrep-3.12 /nix/store/bpy2ps3f0f3gvgvqjwgqriw1j1n840wd-gawk-5.4.1 /nix/store/w2mbg6mcq7k3j088sbwfziy8vsdhyk7v-gnutar-1.35 /nix/store/sd9qcv5s2gxvd6hpp5hk5cixq2bd4jd1-gzip-1.14 /nix/store/622jkvg8hgffly7dca6iif9drvimingx-bzip2-1.0.8-bin /nix/store/hd36xbvbc0vrn9f9idrfwxznri0ax427-gnumake-4.4.1 /nix/store/9ipfvwnqp1q8ijnmi5sxvlx9r8w34lw3-bash-5.3p15 /nix/store/1rcr9yyj9qlgm3dyin1j37xl7q84h15i-patch-2.8 /nix/store/0fb3jrf20z28598alm6g7rrsw6dz2cqj-xz-5.8.3-bin /nix/store/xcab8m6wdrd30l10bw3hkz32yiibynzv-file-5.48'
preInstallPhases=' glibPreInstallPhase'
role_post='_FOR_BUILD'
NIX_BINTOOLS_FOR_BUILD='/nix/store/rm982p6i9ry7rwq5jn638kdpv0a2331f-binutils-wrapper-2.46'
export NIX_BINTOOLS_FOR_BUILD
preFixupPhases=' dropIconThemeCache'
declare -a unpackCmdHooks=('_defaultUnpack' )
NIX_CFLAGS_COMPILE=' -frandom-seed=w5zw86r6j0 -isystem /nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev/include -isystem /nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/include -isystem /nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/include -isystem /nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev/include -isystem /nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev/include -isystem /nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev/include -isystem /nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42/include -isystem /nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev/include -isystem /nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/include -isystem /nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/include -isystem /nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev/include -isystem /nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev/include -isystem /nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/include -isystem /nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/include -isystem /nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/include -isystem /nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/include -isystem /nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev/include -isystem /nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev/include -isystem /nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev/include -isystem /nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/include -isystem /nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev/include -isystem /nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/include -isystem /nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/include -isystem /nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/include -isystem /nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev/include -isystem /nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/include -isystem /nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/include -isystem /nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev/include -isystem /nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/include -isystem /nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev/include -isystem /nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev/include -isystem /nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev/include -isystem /nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev/include -isystem /nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev/include -isystem /nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/include -isystem /nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev/include -isystem /nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev/include -isystem /nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev/include -isystem /nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/include -isystem /nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev/include -isystem /nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/include -isystem /nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev/include -isystem /nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/include -isystem /nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev/include -isystem /nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev/include -isystem /nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev/include -isystem /nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev/include -isystem /nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev/include -isystem /nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev/include -isystem /nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev/include -isystem /nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev/include -isystem /nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/include -isystem /nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/include -isystem /nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev/include -isystem /nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev/include -isystem /nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev/include -isystem /nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev/include -isystem /nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/include -isystem /nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev/include -isystem /nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/include -isystem /nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev/include -isystem /nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/include -isystem /nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/include -isystem /nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/include -isystem /nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev/include -isystem /nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/include -isystem /nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev/include -isystem /nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev/include -isystem /nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev/include -isystem /nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev/include -isystem /nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev/include -isystem /nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev/include -isystem /nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/include -isystem /nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/include -isystem /nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/include -isystem /nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev/include -isystem /nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/include -isystem /nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/include -isystem /nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev/include -isystem /nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev/include -isystem /nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev/include -isystem /nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42/include -isystem /nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev/include -isystem /nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/include -isystem /nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/include -isystem /nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev/include -isystem /nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev/include -isystem /nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/include -isystem /nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/include -isystem /nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/include -isystem /nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/include -isystem /nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev/include -isystem /nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev/include -isystem /nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev/include -isystem /nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/include -isystem /nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev/include -isystem /nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/include -isystem /nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/include -isystem /nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/include -isystem /nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev/include -isystem /nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/include -isystem /nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/include -isystem /nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev/include -isystem /nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/include -isystem /nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev/include -isystem /nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev/include -isystem /nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev/include -isystem /nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev/include -isystem /nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev/include -isystem /nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/include -isystem /nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev/include -isystem /nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev/include -isystem /nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev/include -isystem /nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/include -isystem /nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev/include -isystem /nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/include -isystem /nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev/include -isystem /nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/include -isystem /nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev/include -isystem /nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev/include -isystem /nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev/include -isystem /nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev/include -isystem /nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev/include -isystem /nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev/include -isystem /nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev/include -isystem /nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev/include -isystem /nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/include -isystem /nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/include -isystem /nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev/include -isystem /nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev/include -isystem /nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev/include -isystem /nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev/include -isystem /nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/include -isystem /nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev/include -isystem /nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/include -isystem /nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev/include -isystem /nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/include -isystem /nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/include -isystem /nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/include -isystem /nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev/include -isystem /nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/include -isystem /nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev/include -isystem /nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev/include -isystem /nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev/include -isystem /nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev/include -isystem /nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev/include -isystem /nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev/include -isystem /nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/include -isystem /nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/include -isystem /nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/include'
export NIX_CFLAGS_COMPILE
setOutputFlags=''
AR_FOR_BUILD='ar'
export AR_FOR_BUILD
STRIP='strip'
export STRIP
declare -a pkgsTargetTarget=()
configurePhase='cmakeConfigurePhase'
declare -a postUnpackHooks=('_updateSourceDateEpochFromSourceRoot' )
declare -a fixupOutputHooks=('_gtkCleanImmodulesCache' 'if [ -z "${dontPatchELF-}" ]; then patchELF "$prefix"; fi' 'if [[ -z "${noAuditTmpdir-}" && -e "$prefix" ]]; then auditTmpdir "$prefix"; fi' 'if [ -z "${dontGzipMan-}" ]; then compressManPages "$prefix"; fi' '_moveLib64' '_moveSbin' '_moveSystemdUserUnits' 'patchShebangsAuto' '_pruneLibtoolFiles' '_doStrip' )
PKG_CONFIG_PATH='/nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev/lib/pkgconfig:/nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev/lib/pkgconfig:/nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev/lib/pkgconfig:/nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev/lib/pkgconfig:/nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev/lib/pkgconfig:/nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/lib/pkgconfig:/nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/share/pkgconfig:/nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev/lib/pkgconfig:/nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev/lib/pkgconfig:/nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/lib/pkgconfig:/nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/lib/pkgconfig:/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib/pkgconfig:/nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/lib/pkgconfig:/nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev/lib/pkgconfig:/nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev/lib/pkgconfig:/nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev/lib/pkgconfig:/nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/lib/pkgconfig:/nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev/lib/pkgconfig:/nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/lib/pkgconfig:/nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/lib/pkgconfig:/nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev/lib/pkgconfig:/nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/lib/pkgconfig:/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/lib/pkgconfig:/nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev/lib/pkgconfig:/nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/share/pkgconfig:/nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev/lib/pkgconfig:/nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev/lib/pkgconfig:/nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev/lib/pkgconfig:/nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev/lib/pkgconfig:/nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev/lib/pkgconfig:/nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/lib/pkgconfig:/nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev/lib/pkgconfig:/nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev/lib/pkgconfig:/nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev/lib/pkgconfig:/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/lib/pkgconfig:/nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev/lib/pkgconfig:/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/lib/pkgconfig:/nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev/lib/pkgconfig:/nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/share/pkgconfig:/nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev/lib/pkgconfig:/nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev/lib/pkgconfig:/nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev/lib/pkgconfig:/nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev/lib/pkgconfig:/nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev/lib/pkgconfig:/nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev/lib/pkgconfig:/nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev/lib/pkgconfig:/nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev/lib/pkgconfig:/nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/lib/pkgconfig:/nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/lib/pkgconfig:/nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev/lib/pkgconfig:/nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev/lib/pkgconfig:/nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev/lib/pkgconfig:/nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev/lib/pkgconfig:/nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/share/pkgconfig:/nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev/lib/pkgconfig:/nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/lib/pkgconfig:/nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev/lib/pkgconfig:/nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/lib/pkgconfig:/nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/lib/pkgconfig:/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib/pkgconfig:/nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev/lib/pkgconfig:/nix/store/v3cjb49fng33cddf41nj18z5idlkj86a-vulkan-loader-1.4.357.0-dev/lib/pkgconfig:/nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/lib/pkgconfig:/nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev/lib/pkgconfig:/nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev/lib/pkgconfig:/nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev/lib/pkgconfig:/nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev/lib/pkgconfig:/nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev/lib/pkgconfig:/nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev/lib/pkgconfig:/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/lib/pkgconfig:/nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/lib/pkgconfig:/nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/share/pkgconfig'
export PKG_CONFIG_PATH
declare -a pkgsBuildBuild=('/nix/store/w88q44gqd1qg5wmkk7v0h97rpiqvam0l-gcc-wrapper-15.3.0' '/nix/store/rm982p6i9ry7rwq5jn638kdpv0a2331f-binutils-wrapper-2.46' )
PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS='true'
export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS
IFS=' 	
'
DEVENV_TASKS=''
export DEVENV_TASKS
out='/nix/store/w5zw86r6j0cy07bbkdwa11icymwypi9v-devenv-shell-env'
export out
NIX_PKG_CONFIG_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu='1'
export NIX_PKG_CONFIG_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu
CXX='g++'
export CXX
allowSubstitutes=''
export allowSubstitutes
mesonFlags=''
export mesonFlags
DEVENV_RUNTIME='/run/user/1000/devenv-019e219'
export DEVENV_RUNTIME
propagatedNativeBuildInputs=''
export propagatedNativeBuildInputs
shell='/nix/store/9ipfvwnqp1q8ijnmi5sxvlx9r8w34lw3-bash-5.3p15/bin/bash'
export shell
system='x86_64-linux'
export system
declare -a preFixupHooks=('_moveToShare' '_multioutDocs' '_multioutDevs' )
NIX_CC_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu='1'
export NIX_CC_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu
AR_wasm32_unknown_unknown='/nix/store/p7x4y22a9gs20k729qjsz12nsilkhfqi-llvm-binutils-18.1.8/bin/llvm-ar'
export AR_wasm32_unknown_unknown
nativeBuildInputs='/nix/store/4pgr1yxmpsp0dsfvsakaf0cyyv2zhrqx-fleet-doctor /nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev /nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1 /nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1 /nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8 /nix/store/dvj9bnk0vdbzzhn76wsnzk1dv6c6jbx1-stdenv-linux /nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1 /nix/store/6jh160z5zby7p11nk8x7z3jdnxmimpjp-ccls-0.20250815.1 /nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2 /nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev /nix/store/p6s1f49k1s63r79bmqfvynsbbbrqvnca-cargo-watch-8.5.3 /nix/store/wfk7hizfxni5fyaqg3x5pwnpy0qf5d13-cargo-nextest-0.9.143 /nix/store/1gw6in1ws9y9vlh0frhxzixm3k7y7v08-bacon-3.25.0 /nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev /nix/store/cbj4qx4v111j2pw218s1sffl0lsijpb9-uv-0.12.11 /nix/store/bpw06lnk816nznyc9gwmpia5mk5fs5ac-cargo-nightly /nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev /nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev /nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42 /nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev /nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev /nix/store/908kvhj55s5z7laavf4pddxrxl62rrgl-cmake-4.3.4 /nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7 /nix/store/g4dayjlkziardmjyxpmjpv9pmlbzaadi-rust-bindgen-hook /nix/store/8yjpgadavqdwphcd2kw4dfvxmgdp5sb3-git-2.55.0 /nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev /nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev /nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev /nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev /nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev /nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev /nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev /nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev /nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev /nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev /nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev /nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev /nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev /nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev /nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev /nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1 /nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev /nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev /nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev /nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev /nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev /nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev /nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev /nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev /nix/store/v3cjb49fng33cddf41nj18z5idlkj86a-vulkan-loader-1.4.357.0-dev /nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack /nix/store/dniq5gd7rj0jb9g8mk3c7phfpfdxrzci-jack-example-tools-4 /nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev /nix/store/81y35hm10qdnb0s9lbfmy12vbcgnnvv3-xvfb-run-1+g87f6705 /nix/store/yfaj060r9jn5wxlknnnnm22h9p0ahnbm-cargo-sweep-0.8.0 /nix/store/4qmwq1p9crkyg97sk14q8g5shxb1cf7r-dioxus-cli-0.8.0-alpha.0 /nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129 /nix/store/v8rz35q5sk5m2wln3h75abnmymags0p1-wasm-bindgen-cli-0.2.127 /nix/store/cslxcx72gg4lzgnc8nwfbp5gl9hznap0-tailwindcss_4-4.3.3 /nix/store/6dhcgiidbg7ldz861yl8jgcj9ax8liyi-cargo-hakari-0.9.38 /nix/store/z4czsax3mdyxx77mwb0yjarnzb1rip00-just-1.58.0 /nix/store/rag8h8gpcmksr927bc4y5bgfsb879kmr-prek-0.5.7 /nix/store/2cy8618y6n260ww0j77gbd6cz77y1har-cargo-rail-0.32.0 /nix/store/7fx93fz9c809w8yjxb0934zw6xcjis58-pkg-config-wrapper-0.29.2 /nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev /nix/store/7fx93fz9c809w8yjxb0934zw6xcjis58-pkg-config-wrapper-0.29.2 /nix/store/4qlkb64qf364lsygwadiiyjjbc0hcq6i-mold-unwrapped-wrapper-2.42.0 /nix/store/7fx93fz9c809w8yjxb0934zw6xcjis58-pkg-config-wrapper-0.29.2'
export nativeBuildInputs
maybe_dir='/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/share/gsettings-schemas/gtk+3-3.24.52'
SIZE='size'
export SIZE
depsBuildBuild=''
export depsBuildBuild
outputInfo='out'
phases='buildPhase'
export phases
LD_FOR_BUILD='ld'
export LD_FOR_BUILD
OPENSSL_LIB_DIR='/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib'
export OPENSSL_LIB_DIR
MACHTYPE='x86_64-pc-linux-gnu'
NM='nm'
export NM
READELF='readelf'
export READELF
NIX_BUILD_CORES='4'
export NIX_BUILD_CORES
declare -a propagatedTargetDepFiles=('propagated-target-target-deps' )
LIBCLANG_PATH='/nix/store/yc2a9854a2y2c8kci88piblc847iq1l4-clang-21.1.8-lib/lib'
export LIBCLANG_PATH
declare -a postInstallHooks=('glibPostInstallHook' )
preferLocalBuild='1'
export preferLocalBuild
NIX_NO_SELF_RPATH='1'
declare -a postHooks=('makeCmakeFindLibs' )
CONFIG_SHELL='/nix/store/9ipfvwnqp1q8ijnmi5sxvlx9r8w34lw3-bash-5.3p15/bin/bash'
export CONFIG_SHELL
PYTHONNOUSERSITE='1'
export PYTHONNOUSERSITE
GETTEXTDATADIRS_FOR_BUILD='/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/share/gettext:/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/share/gettext:/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/share/gettext'
export GETTEXTDATADIRS_FOR_BUILD
OBJDUMP='objdump'
export OBJDUMP
CMAKE_LIBRARY_PATH='/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/lib:/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8/lib:/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/lib:/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0/lib:/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib:/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2/lib:/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8/lib:/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib/lib:/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58/lib:/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/lib:/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3/lib:/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/lib:/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib:/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/lib:/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/lib:/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22/lib:/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/lib:/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0/lib:/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/lib:/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/lib:/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib/lib:/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib:/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/lib:/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12/lib:/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7/lib:/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/lib:/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12/lib:/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0/lib:/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4/lib:/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16/lib:/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1/lib:/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/lib:/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/lib:/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3/lib:/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/lib:/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0/lib:/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2/lib:/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib:/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2/lib:/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6/lib:/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2/lib:/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7/lib:/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3/lib:/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7/lib:/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3/lib:/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5/lib:/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/lib:/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1/lib:/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9/lib:/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1/lib:/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0/lib:/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0/lib:/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6/lib:/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0/lib:/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib/lib:/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/lib:/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/lib:/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/lib:/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib:/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/lib:/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0/lib:/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack/lib:/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10/lib:/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3/lib:/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3/lib:/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4/lib:/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9/lib:/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3/lib:/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/lib:/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2/lib:/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/lib'
export CMAKE_LIBRARY_PATH
LD='ld'
export LD
strictDeps=''
export strictDeps
declare -a propagatedHostDepFiles=('propagated-host-host-deps' 'propagated-build-inputs' )
NIX_LDFLAGS='-rpath /nix/store/w5zw86r6j0cy07bbkdwa11icymwypi9v-devenv-shell-env/lib  -L/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/lib -L/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/lib -L/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8/lib -L/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8/lib -L/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/lib -L/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/lib -L/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0/lib -L/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0/lib -L/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib -L/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib -L/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2/lib -L/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2/lib -L/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8/lib -L/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8/lib -L/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib/lib -L/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib/lib -L/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58/lib -L/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58/lib -L/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/lib -L/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/lib -L/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3/lib -L/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3/lib -L/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/lib -L/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/lib -L/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib -L/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib -L/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/lib -L/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/lib -L/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/lib -L/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/lib -L/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22/lib -L/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22/lib -L/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/lib -L/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/lib -L/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0/lib -L/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0/lib -L/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/lib -L/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/lib -L/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/lib -L/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/lib -L/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib/lib -L/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib/lib -L/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib -L/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib -L/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/lib -L/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/lib -L/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12/lib -L/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12/lib -L/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7/lib -L/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7/lib -L/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/lib -L/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/lib -L/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12/lib -L/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12/lib -L/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0/lib -L/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0/lib -L/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4/lib -L/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4/lib -L/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16/lib -L/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16/lib -L/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1/lib -L/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1/lib -L/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/lib -L/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/lib -L/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/lib -L/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/lib -L/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3/lib -L/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3/lib -L/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/lib -L/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/lib -L/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0/lib -L/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0/lib -L/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2/lib -L/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2/lib -L/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib -L/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib -L/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2/lib -L/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2/lib -L/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6/lib -L/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6/lib -L/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2/lib -L/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2/lib -L/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7/lib -L/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7/lib -L/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3/lib -L/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3/lib -L/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7/lib -L/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7/lib -L/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3/lib -L/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3/lib -L/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5/lib -L/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5/lib -L/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/lib -L/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/lib -L/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1/lib -L/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1/lib -L/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9/lib -L/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9/lib -L/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1/lib -L/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1/lib -L/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0/lib -L/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0/lib -L/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0/lib -L/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0/lib -L/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6/lib -L/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6/lib -L/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0/lib -L/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0/lib -L/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib/lib -L/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib/lib -L/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/lib -L/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/lib -L/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/lib -L/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/lib -L/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/lib -L/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/lib -L/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib -L/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib -L/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/lib -L/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/lib -L/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0/lib -L/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0/lib -L/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack/lib -L/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack/lib -L/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10/lib -L/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10/lib -L/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3/lib -L/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3/lib -L/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3/lib -L/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3/lib -L/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4/lib -L/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4/lib -L/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9/lib -L/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9/lib -L/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3/lib -L/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3/lib -L/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/lib -L/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/lib -L/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2/lib -L/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2/lib -L/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/lib -L/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/lib -L/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/lib -L/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/lib -L/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8/lib -L/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8/lib -L/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/lib -L/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/lib -L/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0/lib -L/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0/lib -L/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib -L/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3/lib -L/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2/lib -L/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2/lib -L/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8/lib -L/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8/lib -L/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib/lib -L/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib/lib -L/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58/lib -L/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58/lib -L/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/lib -L/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/lib -L/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3/lib -L/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3/lib -L/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/lib -L/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/lib -L/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib -L/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib -L/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/lib -L/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/lib -L/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/lib -L/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/lib -L/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22/lib -L/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22/lib -L/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/lib -L/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/lib -L/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0/lib -L/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0/lib -L/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/lib -L/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/lib -L/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/lib -L/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/lib -L/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib/lib -L/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib/lib -L/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib -L/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib -L/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/lib -L/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4/lib -L/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12/lib -L/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12/lib -L/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7/lib -L/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7/lib -L/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/lib -L/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/lib -L/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12/lib -L/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12/lib -L/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0/lib -L/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0/lib -L/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4/lib -L/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4/lib -L/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16/lib -L/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16/lib -L/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1/lib -L/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1/lib -L/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/lib -L/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/lib -L/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/lib -L/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/lib -L/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3/lib -L/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3/lib -L/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/lib -L/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/lib -L/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0/lib -L/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0/lib -L/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2/lib -L/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2/lib -L/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib -L/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib -L/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2/lib -L/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2/lib -L/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6/lib -L/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6/lib -L/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2/lib -L/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2/lib -L/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7/lib -L/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7/lib -L/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3/lib -L/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3/lib -L/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7/lib -L/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7/lib -L/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3/lib -L/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3/lib -L/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5/lib -L/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5/lib -L/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/lib -L/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/lib -L/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1/lib -L/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1/lib -L/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9/lib -L/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9/lib -L/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1/lib -L/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1/lib -L/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0/lib -L/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0/lib -L/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0/lib -L/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0/lib -L/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6/lib -L/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6/lib -L/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0/lib -L/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0/lib -L/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib/lib -L/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib/lib -L/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/lib -L/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/lib -L/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/lib -L/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/lib -L/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/lib -L/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/lib -L/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib -L/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/lib -L/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/lib -L/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/lib -L/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0/lib -L/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0/lib -L/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack/lib -L/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack/lib -L/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10/lib -L/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10/lib -L/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3/lib -L/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3/lib -L/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3/lib -L/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3/lib -L/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4/lib -L/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4/lib -L/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9/lib -L/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9/lib -L/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3/lib -L/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3/lib -L/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/lib -L/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19/lib -L/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2/lib -L/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2/lib -L/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/lib -L/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/lib'
export NIX_LDFLAGS
buildInputs=''
export buildInputs
declare -a pkgsBuildTarget=()
LINENO='79'
RANLIB='ranlib'
export RANLIB
AS='as'
export AS
CC_wasm32_unknown_unknown='/nix/store/gby6k5yayznk6339rqf9crwbafzppr4w-clang-18.1.8/bin/clang'
export CC_wasm32_unknown_unknown
NIX_CC='/nix/store/w88q44gqd1qg5wmkk7v0h97rpiqvam0l-gcc-wrapper-15.3.0'
export NIX_CC
CC='gcc'
export CC
READELF_FOR_BUILD='readelf'
export READELF_FOR_BUILD
STRINGS_FOR_BUILD='strings'
export STRINGS_FOR_BUILD
depsBuildTargetPropagated=''
export depsBuildTargetPropagated
outputDevdoc='REMOVE'
CC_FOR_BUILD='gcc'
export CC_FOR_BUILD
STRIP_FOR_BUILD='strip'
export STRIP_FOR_BUILD
declare -a pkgsHostHost=()
declare -a envHostTargetHooks=('ccWrapper_addCVars' 'bintoolsWrapper_addLDVars' 'addCMakeParams' 'make_glib_find_gsettings_schemas' 'findGdkPixbufLoaders' 'pkgConfigWrapper_addPkgConfigPath' 'bintoolsWrapper_addLDVars' )
PS4='+ '
DEVENV_DOTFILE='/run/media/Development/wt/keyflow-control/.devenv'
export DEVENV_DOTFILE
CXX_FOR_BUILD='g++'
export CXX_FOR_BUILD
BASH='/nix/store/9ipfvwnqp1q8ijnmi5sxvlx9r8w34lw3-bash-5.3p15/bin/bash'
NM_FOR_BUILD='nm'
export NM_FOR_BUILD
NIX_ENFORCE_NO_NATIVE='1'
export NIX_ENFORCE_NO_NATIVE
_PYTHON_SYSCONFIGDATA_NAME='_sysconfigdata__linux_x86_64-linux-gnu'
export _PYTHON_SYSCONFIGDATA_NAME
outputInclude='out'
prefix='/nix/store/w5zw86r6j0cy07bbkdwa11icymwypi9v-devenv-shell-env'
OLDPWD=''
export OLDPWD
outputDoc='out'
NIXPKGS_CMAKE_PREFIX_PATH='/nix/store/w88q44gqd1qg5wmkk7v0h97rpiqvam0l-gcc-wrapper-15.3.0:/nix/store/rm982p6i9ry7rwq5jn638kdpv0a2331f-binutils-wrapper-2.46:/nix/store/4pgr1yxmpsp0dsfvsakaf0cyyv2zhrqx-fleet-doctor:/nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev:/nix/store/7a60q5dgnv6z96c279rc1nalyiw4mgqn-bash-interactive-5.3p15:/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1:/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8:/nix/store/dvj9bnk0vdbzzhn76wsnzk1dv6c6jbx1-stdenv-linux:/nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1:/nix/store/6jh160z5zby7p11nk8x7z3jdnxmimpjp-ccls-0.20250815.1:/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2:/nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev:/nix/store/7p6qjr2kqkbd0i3ylndwrxsb0j7m317k-valgrind-3.27.1:/nix/store/p6s1f49k1s63r79bmqfvynsbbbrqvnca-cargo-watch-8.5.3:/nix/store/wfk7hizfxni5fyaqg3x5pwnpy0qf5d13-cargo-nextest-0.9.143:/nix/store/1gw6in1ws9y9vlh0frhxzixm3k7y7v08-bacon-3.25.0:/nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev:/nix/store/l6ww8kbj9g5nflwp1m04l7in832z9zsw-flac-1.5.0-bin:/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0:/nix/store/cbj4qx4v111j2pw218s1sffl0lsijpb9-uv-0.12.11:/nix/store/bpw06lnk816nznyc9gwmpia5mk5fs5ac-cargo-nightly:/nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev:/nix/store/63b5kdhvzs4sq7xgg8bikkvdv3kf7c7p-openssl-3.6.3-bin:/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3:/nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42:/nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev:/nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev:/nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev:/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2:/nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev:/nix/store/622jkvg8hgffly7dca6iif9drvimingx-bzip2-1.0.8-bin:/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8:/nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev:/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib:/nix/store/34vidkz6jm5pinhhyxi64pj82x70vida-brotli-1.2.0:/nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev:/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58:/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3:/nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev:/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3:/nix/store/122fcxcqy3ay3jkm3fa2bav0vb8mgdjy-fontconfig-2.18.2-bin:/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib:/nix/store/908kvhj55s5z7laavf4pddxrxl62rrgl-cmake-4.3.4:/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7:/nix/store/g4dayjlkziardmjyxpmjpv9pmlbzaadi-rust-bindgen-hook:/nix/store/8yjpgadavqdwphcd2kw4dfvxmgdp5sb3-git-2.55.0:/nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev:/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1:/nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev:/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1:/nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev:/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22:/nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev:/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8:/nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev:/nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev:/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0:/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0:/nix/store/qyknva7lkacxsgpdpgcqy5kxshwwlp2x-glib-2.88.3-bin:/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3:/nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev:/nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev:/nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev:/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib:/nix/store/2swbxihjql78pnn5kfmhjq5dvp9m744b-dbus-1.16.2:/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6:/nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev:/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4:/nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev:/nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1:/nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev:/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12:/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7:/nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev:/nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev:/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13:/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12:/nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev:/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0:/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4:/nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev:/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16:/nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev:/nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev:/nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev:/nix/store/7g0vbmal61bq8bw1vvrcjhrps94kikna-libjpeg-turbo-3.1.4.1-bin:/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1:/nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev:/nix/store/npsbrz5sr41nk35fap059a8ps05wjj63-zstd-1.5.7-bin:/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7:/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25:/nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev:/nix/store/0fb3jrf20z28598alm6g7rrsw6dz2cqj-xz-5.8.3-bin:/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3:/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0:/nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev:/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0:/nix/store/lmxwk264y51a47h1h5zk3phfsqqzw9cz-libtiff-4.7.2-bin:/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2:/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7:/nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1:/nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev:/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2:/nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev:/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6:/nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev:/nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev:/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2:/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7:/nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev:/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3:/nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev:/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7:/nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev:/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3:/nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev:/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5:/nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev:/nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev:/nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev:/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15:/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1:/nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev:/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9:/nix/store/h81cqwzp08wcmnqimammsfqxnf0ghk5f-pango-1.57.1-bin:/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1:/nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev:/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0:/nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev:/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0:/nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49:/nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev:/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6:/nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev:/nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev:/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0:/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib:/nix/store/ns769gwc5z8a1vlkiyz3s1c77g97yksc-cups-2.4.19:/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52:/nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev:/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6:/nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev:/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1:/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1:/nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev:/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2:/nix/store/v3cjb49fng33cddf41nj18z5idlkj86a-vulkan-loader-1.4.357.0-dev:/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0:/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack:/nix/store/dniq5gd7rj0jb9g8mk3c7phfpfdxrzci-jack-example-tools-4:/nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev:/nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev:/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10:/nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev:/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3:/nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev:/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3:/nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev:/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4:/nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev:/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9:/nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev:/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3:/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19:/nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev:/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2:/nix/store/5w897qjkd3wgx28h8gnzk2zah213g6lf-xorg-server-21.1.24:/nix/store/81y35hm10qdnb0s9lbfmy12vbcgnnvv3-xvfb-run-1+g87f6705:/nix/store/yfaj060r9jn5wxlknnnnm22h9p0ahnbm-cargo-sweep-0.8.0:/nix/store/4qmwq1p9crkyg97sk14q8g5shxb1cf7r-dioxus-cli-0.8.0-alpha.0:/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129:/nix/store/v8rz35q5sk5m2wln3h75abnmymags0p1-wasm-bindgen-cli-0.2.127:/nix/store/cslxcx72gg4lzgnc8nwfbp5gl9hznap0-tailwindcss_4-4.3.3:/nix/store/6dhcgiidbg7ldz861yl8jgcj9ax8liyi-cargo-hakari-0.9.38:/nix/store/z4czsax3mdyxx77mwb0yjarnzb1rip00-just-1.58.0:/nix/store/rag8h8gpcmksr927bc4y5bgfsb879kmr-prek-0.5.7:/nix/store/2cy8618y6n260ww0j77gbd6cz77y1har-cargo-rail-0.32.0:/nix/store/7fx93fz9c809w8yjxb0934zw6xcjis58-pkg-config-wrapper-0.29.2:/nix/store/4qlkb64qf364lsygwadiiyjjbc0hcq6i-mold-unwrapped-wrapper-2.42.0:/nix/store/fpig164lz7fcipbndn61sfd2y2yq363n-patchelf-0.15.2:/nix/store/7n9ldpw06qbqlnf0iaig26agpn2aazfl-update-autotools-gnu-config-scripts-hook'
export NIXPKGS_CMAKE_PREFIX_PATH
depsTargetTargetPropagated=''
export depsTargetTargetPropagated
NIX_HARDENING_ENABLE='bindnow format fortify fortify3 libcxxhardeningfast pic relro stackclashprotection stackprotector strictflexarrays1 strictoverflow zerocallusedregs'
export NIX_HARDENING_ENABLE
AS_FOR_BUILD='as'
export AS_FOR_BUILD
NIX_CC_FOR_BUILD='/nix/store/w88q44gqd1qg5wmkk7v0h97rpiqvam0l-gcc-wrapper-15.3.0'
export NIX_CC_FOR_BUILD
STRINGS='strings'
export STRINGS
flag='-L/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/lib'
stdenv='/nix/store/dvj9bnk0vdbzzhn76wsnzk1dv6c6jbx1-stdenv-linux'
export stdenv
outputMan='out'
shellHook='


# Override temp directories that stdenv set to NIX_BUILD_TOP.
# Only reset those that still point to the Nix build dir; leave
# any user/CI-supplied value intact.
for var in TMP TMPDIR TEMP TEMPDIR; do
  if [ -n "${!var-}" ] && [ "${!var}" = "${NIX_BUILD_TOP-}" ]; then
    export "$var"=/tmp
  fi
done
if [ -n "${NIX_BUILD_TOP-}" ]; then
  unset NIX_BUILD_TOP
fi

# set path to locales on non-NixOS Linux hosts
if [ -z "${LOCALE_ARCHIVE-}" ]; then
  export LOCALE_ARCHIVE=/nix/store/lcgpmpgqjmsb9m69lijkqpdlf5k23csl-glibc-locales-2.42-84/lib/locale/locale-archive
fi


# Make `man <tool>` find the man pages of packages in the profile.
export MANPATH="$DEVENV_PROFILE/share/man:${MANPATH:+$MANPATH:}"

# direnv helper
if [ ! type -p direnv &>/dev/null && -f .envrc ]; then
  echo "An .envrc file was detected, but the direnv command is not installed."
  echo "To use this configuration, please install direnv: https://direnv.net/docs/installation.html"
fi

mkdir -p "$DEVENV_STATE"
if [ ! -L "$DEVENV_DOTFILE/profile" ] || [ "$(/nix/store/3qgy8q2j64v2m9jy3a5jmssacbblhd4r-coreutils-9.11/bin/readlink $DEVENV_DOTFILE/profile)" != "/nix/store/0lw8hnf5wmymp2ghd9ybw1ib5ncjixzn-devenv-profile" ]
then
  ln -snf /nix/store/0lw8hnf5wmymp2ghd9ybw1ib5ncjixzn-devenv-profile "$DEVENV_DOTFILE/profile"
fi
unset 

mkdir -p /run/user/1000/devenv-019e219
ln -snf /run/user/1000/devenv-019e219 /run/media/Development/wt/keyflow-control/.devenv/run

export CARGO_INSTALL_ROOT=$(/nix/store/3qgy8q2j64v2m9jy3a5jmssacbblhd4r-coreutils-9.11/bin/realpath --no-symlinks /run/media/Development/wt/keyflow-control/.devenv/state/cargo-install)
export PATH="$PATH:$CARGO_INSTALL_ROOT/bin"
export PATH="$PATH:${CARGO_HOME:-$HOME/.cargo}/bin"



# Check whether the direnv integration is out of date.
{
  if [[ ":${DIRENV_ACTIVE-}:" == *":/run/media/Development/wt/keyflow-control:"* ]]; then
    if [[ ! "${DEVENV_NO_DIRENVRC_OUTDATED_WARNING-}" == 1 && ! "${DEVENV_DIRENVRC_ROLLING_UPGRADE-}" == 1 ]]; then
      if [[ ${DEVENV_DIRENVRC_VERSION:-0} -lt 2 ]]; then
        direnv_line=$(grep --color=never -E "source_url.*cachix/devenv" .envrc || echo "")

        echo "✨ The direnv integration in your .envrc is out of date."
        echo ""
        echo -n "RECOMMENDED: devenv can now auto-upgrade the direnv integration. "
        if [[ -n "$direnv_line" ]]; then
          echo "To enable this feature, replace the following line in your .envrc:"
          echo ""
          echo "  $direnv_line"
          echo ""
          echo "with:"
          echo ""
          echo "  eval \"\$(devenv direnvrc)\""
        else
          echo "To enable this feature, replace the \`source_url\` line that fetches the direnvrc integration in your .envrc with:"
          echo ""
          echo "  eval \"$(devenv direnvrc)\""
        fi
        echo ""
          echo "If you prefer to continue managing the integration manually, follow the upgrade instructions at https://devenv.sh/integrations/direnv/."
          echo ""
          echo "To disable this message:"
          echo ""
          echo "  Add the following environment to your .envrc before \`use devenv\`:"
          echo ""
          echo "    export DEVENV_NO_DIRENVRC_OUTDATED_WARNING=1"
          echo ""
          echo "  Or set the following option in your devenv configuration:"
          echo ""
          echo "    devenv.warnOnNewVersion = false;"
          echo ""
      fi
    fi
  fi
} >&2

# Machine-local values (.env, not committed) go into the shell'\''s environment.
if [ -f "$DEVENV_ROOT/.env" ]; then set -a; . "$DEVENV_ROOT/.env"; set +a; fi

# Appended, not prepended: store-provided tools must win over stale
# cargo-installed copies; cargo and uv bins only need to be reachable.
export PATH="$PATH:$HOME/.cargo/bin:$HOME/.local/bin"

# Dev-convenience installers are for an interactive shell only. In CI, or
# under direnv, `cargo install tracey` would compile from source each time.
if [ -t 1 ] && [ -z "${CI:-}" ]; then
  # graphify — whole-repo knowledge graph (safishamsi/graphify), pinned.
  # Code extraction is local (tree-sitter); the [mcp] extra serves it.
  GRAPHIFY_VERSION="0.9.15"
  if ! uv tool list 2>/dev/null | grep -q "graphifyy v$GRAPHIFY_VERSION"; then
    echo "  Installing graphify $GRAPHIFY_VERSION..."
    UV_PYTHON_DOWNLOADS=never uv tool install --force \
      --python "/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/bin/python3" \
      "graphifyy[mcp]==$GRAPHIFY_VERSION" >/dev/null 2>&1 || \
      echo "  (graphify install failed — run: uv tool install '\''graphifyy[mcp]==$GRAPHIFY_VERSION'\'')"
  fi

  # tracey — spec-coverage CLI (crates.io), pinned. Config: .config/tracey/config.styx.
  TRACEY_VERSION=$(tracey --version 2>/dev/null | grep -oE '\''[0-9]+\.[0-9]+\.[0-9]+'\'' | head -1 || echo "0")
  if [ "$TRACEY_VERSION" != "1.3.0" ]; then
    echo "  Installing tracey 1.3.0..."
    cargo install tracey --locked --version "=1.3.0" 2>/dev/null || true
  fi
fi

if [ -z "${RUSTC_WRAPPER:-}" ] && kache=$(command -v kache); then
  export RUSTC_WRAPPER="$kache"
fi
if [ -t 1 ] && [ -t 2 ] && [[ $- == *i* ]] && [ -z "${CI:-}" ] && [ -z "${CLAUDECODE:-}" ]; then
  echo "devenv-fleet: rust 1.99.1, mold, kache via ${RUSTC_WRAPPER:-cargo config}"
  printf '\''%s\n'\'' '\''Keyflow: just check / just test / just ci, just kf --help (the keyflow CLI)
just graph (re)builds the repo knowledge graph
'\''
fi

'
export shellHook
GDK_PIXBUF_MODULE_FILE='/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache'
export GDK_PIXBUF_MODULE_FILE
declare -a envBuildBuildHooks=('ccWrapper_addCVars' 'bintoolsWrapper_addLDVars' 'addPythonPath' 'sysconfigdataHook' 'gettextDataDirsHook' )
declare -a envHostHostHooks=('ccWrapper_addCVars' 'bintoolsWrapper_addLDVars' 'addCMakeParams' 'make_glib_find_gsettings_schemas' 'findGdkPixbufLoaders' 'pkgConfigWrapper_addPkgConfigPath' 'bintoolsWrapper_addLDVars' )
DETERMINISTIC_BUILD='1'
export DETERMINISTIC_BUILD
NIX_BINTOOLS='/nix/store/4qlkb64qf364lsygwadiiyjjbc0hcq6i-mold-unwrapped-wrapper-2.42.0'
export NIX_BINTOOLS
pkg='/nix/store/w88q44gqd1qg5wmkk7v0h97rpiqvam0l-gcc-wrapper-15.3.0'
RUST_SRC_PATH='/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/lib/rustlib/src/rust/library'
export RUST_SRC_PATH
DEVENV_TASK_FILE='/nix/store/380056z3phidz4bas6qza4ys09jyqzrd-tasks.json'
export DEVENV_TASK_FILE
declare -a pkgsHostTarget=()
SOURCE_DATE_EPOCH='315532800'
export SOURCE_DATE_EPOCH
iframework_seen=''
PYTHONPATH='/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/lib/python3.14/site-packages:/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/lib/python3.14/site-packages'
export PYTHONPATH
cmakeFlags=''
export cmakeFlags
XDG_DATA_DIRS='/nix/store/7a60q5dgnv6z96c279rc1nalyiw4mgqn-bash-interactive-5.3p15/share:/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/share:/nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/share:/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/share:/nix/store/1gw6in1ws9y9vlh0frhxzixm3k7y7v08-bacon-3.25.0/share:/nix/store/cbj4qx4v111j2pw218s1sffl0lsijpb9-uv-0.12.11/share:/nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/share:/nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev/share:/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2/share:/nix/store/34vidkz6jm5pinhhyxi64pj82x70vida-brotli-1.2.0/share:/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3/share:/nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/share:/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3/share:/nix/store/122fcxcqy3ay3jkm3fa2bav0vb8mgdjy-fontconfig-2.18.2-bin/share:/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib/share:/nix/store/908kvhj55s5z7laavf4pddxrxl62rrgl-cmake-4.3.4/share:/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/share:/nix/store/8yjpgadavqdwphcd2kw4dfvxmgdp5sb3-git-2.55.0/share:/nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev/share:/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/share:/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/share:/nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/share:/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/share:/nix/store/qyknva7lkacxsgpdpgcqy5kxshwwlp2x-glib-2.88.3-bin/share:/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3/share:/nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/share:/nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev/share:/nix/store/2swbxihjql78pnn5kfmhjq5dvp9m744b-dbus-1.16.2/share:/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6/share:/nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1/share:/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12/share:/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13/share:/nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/share:/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/share:/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3/share:/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/share:/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/share:/nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/share:/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2/share:/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7/share:/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3/share:/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5/share:/nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev/share:/nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/share:/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/share:/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9/share:/nix/store/h81cqwzp08wcmnqimammsfqxnf0ghk5f-pango-1.57.1-bin/share:/nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49/share:/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6/share:/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib/share:/nix/store/ns769gwc5z8a1vlkiyz3s1c77g97yksc-cups-2.4.19/share:/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/share:/nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev/share:/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6/share:/nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev/share:/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/share:/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/share:/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/share:/nix/store/dniq5gd7rj0jb9g8mk3c7phfpfdxrzci-jack-example-tools-4/share:/nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev/share:/nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev/share:/nix/store/5w897qjkd3wgx28h8gnzk2zah213g6lf-xorg-server-21.1.24/share:/nix/store/81y35hm10qdnb0s9lbfmy12vbcgnnvv3-xvfb-run-1+g87f6705/share:/nix/store/4qmwq1p9crkyg97sk14q8g5shxb1cf7r-dioxus-cli-0.8.0-alpha.0/share:/nix/store/z4czsax3mdyxx77mwb0yjarnzb1rip00-just-1.58.0/share:/nix/store/7fx93fz9c809w8yjxb0934zw6xcjis58-pkg-config-wrapper-0.29.2/share:/nix/store/fpig164lz7fcipbndn61sfd2y2yq363n-patchelf-0.15.2/share:/nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/share/gsettings-schemas/gsettings-desktop-schemas-50.1:/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/share/gsettings-schemas/gtk+3-3.24.52'
export XDG_DATA_DIRS
_PYTHON_HOST_PLATFORM='linux-x86_64'
export _PYTHON_HOST_PLATFORM
DEVENV_ROOT='/run/media/Development/wt/keyflow-control'
export DEVENV_ROOT
buildPhase='{ echo "------------------------------------------------------------";
  echo " WARNING: the existence of this path is not guaranteed.";
  echo " It is an internal implementation detail for pkgs.mkShell.";
  echo "------------------------------------------------------------";
  echo;
  # Record all build inputs as runtime dependencies
  export;
} >> "$out"
'
export buildPhase
DX_NO_RUSTC_WRAPPER='1'
export DX_NO_RUSTC_WRAPPER
IN_NIX_SHELL='impure'
export IN_NIX_SHELL
depsHostHostPropagated=''
export depsHostHostPropagated
_substituteStream_has_warned_replace_deprecation='false'
doCheck=''
export doCheck
DEVENV_PROFILE='/nix/store/0lw8hnf5wmymp2ghd9ybw1ib5ncjixzn-devenv-profile'
export DEVENV_PROFILE
declare -a pkgsBuildHost=('/nix/store/4pgr1yxmpsp0dsfvsakaf0cyyv2zhrqx-fleet-doctor' '/nix/store/ad0w92xhd7f1gbwzfgpkd0nz1ipijd8h-bash-interactive-5.3p15-dev' '/nix/store/7a60q5dgnv6z96c279rc1nalyiw4mgqn-bash-interactive-5.3p15' '/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1' '/nix/store/w88q44gqd1qg5wmkk7v0h97rpiqvam0l-gcc-wrapper-15.3.0' '/nix/store/rm982p6i9ry7rwq5jn638kdpv0a2331f-binutils-wrapper-2.46' '/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8' '/nix/store/dvj9bnk0vdbzzhn76wsnzk1dv6c6jbx1-stdenv-linux' '/nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1' '/nix/store/6jh160z5zby7p11nk8x7z3jdnxmimpjp-ccls-0.20250815.1' '/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2' '/nix/store/5jhgdsgl541jznvgv2xlhhksnkix2hib-valgrind-3.27.1-dev' '/nix/store/7p6qjr2kqkbd0i3ylndwrxsb0j7m317k-valgrind-3.27.1' '/nix/store/p6s1f49k1s63r79bmqfvynsbbbrqvnca-cargo-watch-8.5.3' '/nix/store/wfk7hizfxni5fyaqg3x5pwnpy0qf5d13-cargo-nextest-0.9.143' '/nix/store/1gw6in1ws9y9vlh0frhxzixm3k7y7v08-bacon-3.25.0' '/nix/store/1jlbwkgifw9w5fxaqkmqdfv6ww1hgs79-flac-1.5.0-dev' '/nix/store/l6ww8kbj9g5nflwp1m04l7in832z9zsw-flac-1.5.0-bin' '/nix/store/8mg2kw000kc0v6i4graafjbwymb96qnz-flac-1.5.0' '/nix/store/cbj4qx4v111j2pw218s1sffl0lsijpb9-uv-0.12.11' '/nix/store/bpw06lnk816nznyc9gwmpia5mk5fs5ac-cargo-nightly' '/nix/store/3zlj0508348qn8y0f45y7f7954amigpp-openssl-3.6.3-dev' '/nix/store/63b5kdhvzs4sq7xgg8bikkvdv3kf7c7p-openssl-3.6.3-bin' '/nix/store/hg41dqig0qxm7shyi0a0xkw29a14i3js-openssl-3.6.3' '/nix/store/m5nwyps66vgzkqk9vnh7yr9ly7z7hhi3-glibc-iconv-2.42' '/nix/store/vwlb36mv7jbar7y76l7kilvln04xqf6n-fontconfig-2.18.2-dev' '/nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev' '/nix/store/nd34kmibbq6y7qmp78rhw38adsj5ffd0-zlib-1.3.2-dev' '/nix/store/l3p6y554mkxi4vipnnp8809kxj0g1lyv-zlib-1.3.2' '/nix/store/9vxrnnhc400s1rgq801wqfdxhzpcl1z1-bzip2-1.0.8-dev' '/nix/store/622jkvg8hgffly7dca6iif9drvimingx-bzip2-1.0.8-bin' '/nix/store/xz5xsq0nihfs15fbknz5w0yblg04760q-bzip2-1.0.8' '/nix/store/46hh240hlj9si28zra66dyc3nj85lynw-brotli-1.2.0-dev' '/nix/store/c56x66gbj3qwv3g7dhadcfiq408q2hay-brotli-1.2.0-lib' '/nix/store/34vidkz6jm5pinhhyxi64pj82x70vida-brotli-1.2.0' '/nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev' '/nix/store/d3f7r44z2px98z479jgf7qdwqn96jgf6-libpng-apng-1.6.58' '/nix/store/dczkv6755chri9gsvacxvqpz9nn2dwld-freetype-2.14.3' '/nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev' '/nix/store/2gkqj0v0msfcagwf5ix199rf1lnwf2a1-expat-2.8.3' '/nix/store/122fcxcqy3ay3jkm3fa2bav0vb8mgdjy-fontconfig-2.18.2-bin' '/nix/store/iv0yvsywa1v35xzdkqz9wdzhg7vshpdd-fontconfig-2.18.2-lib' '/nix/store/908kvhj55s5z7laavf4pddxrxl62rrgl-cmake-4.3.4' '/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7' '/nix/store/g4dayjlkziardmjyxpmjpv9pmlbzaadi-rust-bindgen-hook' '/nix/store/8yjpgadavqdwphcd2kw4dfvxmgdp5sb3-git-2.55.0' '/nix/store/a0wjrqgl2ypixq3iy5l859ziws9n0idq-alsa-lib-1.2.16.1-dev' '/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1' '/nix/store/yw5snz5hwisbpasdg133xk1h13zbk01q-onnxruntime-1.27.1-dev' '/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1' '/nix/store/18mhh3lc20wpzwrg8i0hkyb1gywhxidn-libjack2-1.9.22-dev' '/nix/store/107511xv9ycg8wksjx36y4wywr8bm464-libjack2-1.9.22' '/nix/store/rm0fnbgacda8p36am4n9a03rq83zj942-pipewire-1.6.8-dev' '/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8' '/nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev' '/nix/store/ysxsfbvgz35w24g5c74604rhxvmia47v-libffi-3.8.0-dev' '/nix/store/wb232z007b8qynidq89lli8lb2c4q0r4-libffi-3.8.0' '/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0' '/nix/store/qyknva7lkacxsgpdpgcqy5kxshwwlp2x-glib-2.88.3-bin' '/nix/store/a3hr0l5skscvbkcr7kz3nhi4linz1p71-glib-2.88.3' '/nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev' '/nix/store/p0ijpx5x5kppx8jrpqzk5zk6ksfw55vl-at-spi2-core-2.60.6-dev' '/nix/store/s30pndvs4kh1dk7jxxj4ljn27spyr8fv-dbus-1.16.2-dev' '/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib' '/nix/store/2swbxihjql78pnn5kfmhjq5dvp9m744b-dbus-1.16.2' '/nix/store/iw5kcyxqr3c4w4m2d2yaqxkkis07awr3-at-spi2-core-2.60.6' '/nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev' '/nix/store/8kmh5aaiadz1vfv1nal7vf3x41b2l92j-pixman-0.46.4' '/nix/store/p7qxr05x9dzb0xxxss8iq73vaz8wjzwp-libxext-1.3.7-dev' '/nix/store/y4k7f46xqci803kfbi78sfy09r1a978x-xorgproto-2025.1' '/nix/store/m9n4kl4gv4qp5kyz334fqhk2p9qamljw-libxau-1.0.12-dev' '/nix/store/qdzn5piljhvzcjwxkxfyi0bc0m215wdb-libxau-1.0.12' '/nix/store/m51xyp95w4gnhcbncz6xi0x5519m293i-libxext-1.3.7' '/nix/store/0p5nsfpza8g92hnrmrm1rqvm25l1mlgv-libxrender-0.9.12-dev' '/nix/store/4sh8xds6acpkvb2f1dqh0jsgvgn5p2sy-libx11-1.8.13-dev' '/nix/store/3vlnkr6z7hh86rrn087df45s4cfjxy65-libx11-1.8.13' '/nix/store/avkwd1sfvr31c5ffd06qj0grvyp9pb2w-libxrender-0.9.12' '/nix/store/b3wyx65651qsp5v73kiyljnv5hr4dgv3-libxcb-1.17.0-dev' '/nix/store/j5ml0bwadkmxc6185ilixlw85ywagxgv-libxcb-1.17.0' '/nix/store/imkw2kilahmpk5c2n0img67wq8ij717v-cairo-1.18.4' '/nix/store/lc6lyn1cgr8a2n78hmrqy4c4dz3f95rd-fribidi-1.0.16-dev' '/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16' '/nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev' '/nix/store/8lyz7iilgz3a1jlg4qf02w0xizzmd52i-libtiff-4.7.2-dev' '/nix/store/21np1b2yflzayldrsy3zp2gg5snhkgwz-libjpeg-turbo-3.1.4.1-dev' '/nix/store/7g0vbmal61bq8bw1vvrcjhrps94kikna-libjpeg-turbo-3.1.4.1-bin' '/nix/store/nc95dy5ln9slqmw1s151mk30ks5f6na6-libjpeg-turbo-3.1.4.1' '/nix/store/ghks3c4bpqll9b1909vr47qx892c3zzw-zstd-1.5.7-dev' '/nix/store/npsbrz5sr41nk35fap059a8ps05wjj63-zstd-1.5.7-bin' '/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7' '/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25' '/nix/store/gbzc2cpjmhylnmwljbqa1pwmli5a2n1v-xz-5.8.3-dev' '/nix/store/0fb3jrf20z28598alm6g7rrsw6dz2cqj-xz-5.8.3-bin' '/nix/store/bdwsskqikgdlq94xqh5dvyd8263w995g-xz-5.8.3' '/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0' '/nix/store/8sv07mfqjbbhb0lac3bn2l0qyigafjzi-lerc-4.2.0-dev' '/nix/store/3rynmgnsaz28k69zs2gfqwqnz0ayps5c-lerc-4.2.0' '/nix/store/lmxwk264y51a47h1h5zk3phfsqqzw9cz-libtiff-4.7.2-bin' '/nix/store/66fky3kgvwffycw067rb7yih282ibzzs-libtiff-4.7.2' '/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7' '/nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1' '/nix/store/v43d0sp8hdg0apjm5i0hrdrsladvvri8-libice-1.1.2-dev' '/nix/store/4qgiw1ck8vifizpd6nigd4w4m6raw32n-libice-1.1.2' '/nix/store/8gs2rj4gzdhk9rz09xx9clqkmgm2r277-libsm-1.2.6-dev' '/nix/store/5vghjl8c7nnm814slkizzf2nn94lx0r0-libsm-1.2.6' '/nix/store/gl601sbj8md9f4gn59g43f4rg7fyh0b7-libxcomposite-0.4.7-dev' '/nix/store/xm0lc6hwnczyhm9y23gqdc4ixpzalxji-libxfixes-6.0.2-dev' '/nix/store/i4f1ydn42hii1y1sz1wx2pl6lkqyby1l-libxfixes-6.0.2' '/nix/store/n9p41ljslm8qcpw26mwh9spw75bi0wsm-libxcomposite-0.4.7' '/nix/store/b3v2anf0jhzgapfpj5pc9z9x2jmk7l80-libxcursor-1.2.3-dev' '/nix/store/algvsjnffll23bmirbg4a6aycfavk609-libxcursor-1.2.3' '/nix/store/z8fiaf41l8zbhjsicvdgjilkqzycvlg4-libxdamage-1.1.7-dev' '/nix/store/sgad4zxa77b97mmy5h5pllzwvjd8bm1l-libxdamage-1.1.7' '/nix/store/gm8wkz37gxc0xiyf48k28c0n2a0bv0rs-libxi-1.8.3-dev' '/nix/store/kvg5r55y17ybrs2gccjbj4vdg9sp69ia-libxi-1.8.3' '/nix/store/h0i1gqjmnwb06h30sdpr62vk2mcvsixy-libxrandr-1.5.5-dev' '/nix/store/jvmx7p97pjyz4a67001lg9yhk0g7qbfm-libxrandr-1.5.5' '/nix/store/5848cfwnrv0by7nb25swhzysd73z9mgc-pango-1.57.1-dev' '/nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev' '/nix/store/lq0l3zkbnb1rhrwbgnr2w0svjr8wyc4r-graphite2-1.3.15-dev' '/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15' '/nix/store/bn8sa8p2ss2y03iwsyy5w6nlzmfd9avg-harfbuzz-13.2.1' '/nix/store/fd8hf26hs7hnpvdk4vrfynrc3y5mwvm4-libxft-2.3.9-dev' '/nix/store/za163pxnk1r2n8rrajqby7qiv5ha38cb-libxft-2.3.9' '/nix/store/h81cqwzp08wcmnqimammsfqxnf0ghk5f-pango-1.57.1-bin' '/nix/store/65582sxlfkllmxwp1l4dwbsfzymzyxry-pango-1.57.1' '/nix/store/knyasnbdxksxz6ilfygnrwa08m9n35m7-libglvnd-1.7.0-dev' '/nix/store/rarvfpm927fcbd6b9p0crg09w9r3ywb3-libglvnd-1.7.0' '/nix/store/7svfxf5fpqsn4jjf8r044f65a030f40x-wayland-1.26.0-dev' '/nix/store/vjcwhp0milwa1jqy1inpjxrsh8hjrgwc-wayland-1.26.0' '/nix/store/2nlq43n0gc4s8cbjjxpnfg97kgmk8blh-wayland-protocols-1.49' '/nix/store/b8ibmndpzni0idlfr1crb200y6rkvfby-libxinerama-1.1.6-dev' '/nix/store/fvjava8j30snn73swinnx10x8c6s8sy7-libxinerama-1.1.6' '/nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev' '/nix/store/wjfai7nny77igkc51xj1izpz2gz6j366-gmp-with-cxx-6.3.0-dev' '/nix/store/jwyskbj2mc7izs67ib2msydsh0vbnwl9-gmp-with-cxx-6.3.0' '/nix/store/vjaz8yglcqmbihslm7qj5gkrdz7cd3hi-cups-2.4.19-lib' '/nix/store/ns769gwc5z8a1vlkiyz3s1c77g97yksc-cups-2.4.19' '/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52' '/nix/store/4q27iqwc58inzr8f6hz73a3aksjimgvm-libsoup-3.6.6-dev' '/nix/store/l9mf9scx79cnqg6xf9r8yjily48hfd64-libsoup-3.6.6' '/nix/store/9rwjx3g2zay8j9jjvjikgyg8hsddb1m0-webkitgtk-2.52.6+abi=4.1-dev' '/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1' '/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1' '/nix/store/1sqzajgcm7nk3knr8y2ygdxgsicrh72x-libxkbcommon-1.13.2-dev' '/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2' '/nix/store/v3cjb49fng33cddf41nj18z5idlkj86a-vulkan-loader-1.4.357.0-dev' '/nix/store/lk4614327cn1h9z3m6cvnls95n13m1cj-vulkan-loader-1.4.357.0' '/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack' '/nix/store/dniq5gd7rj0jb9g8mk3c7phfpfdxrzci-jack-example-tools-4' '/nix/store/f7bm6sr9a8p4k6nhwg0svxvpwfm4rj27-xorg-server-21.1.24-dev' '/nix/store/9vvax647fg26lfwr2g3i8mkip6abbqs4-libepoxy-1.5.10-dev' '/nix/store/kfnh089ilp2acq3iyrj2bcf7p0gij1j6-libepoxy-1.5.10' '/nix/store/n2d6ryxidpsxmj0ba7w58abc5ydqrn9r-glu-9.0.3-dev' '/nix/store/ypvganx6xpk136fhlh7kfa38m44i4bh9-glu-9.0.3' '/nix/store/cyxqna29153mr9bynzadaxj450kqc8d1-libunwind-1.8.3-dev' '/nix/store/apqjl4162bpyx7j6v2l2rsma62q8dljx-libunwind-1.8.3' '/nix/store/sznxhvz748g3pg1kbzq0hgq3grqr5jyf-libxfont_1-1.5.4-dev' '/nix/store/7cdj2zqpxb1x8cfl4aby2swmmrra6q7n-libxfont_1-1.5.4' '/nix/store/79dlllqxhvah7h7q09240r5j21qrglr5-libxfont_2-2.0.9-dev' '/nix/store/qn2i8h6wx6rwc1ckfa7fa0yl8n4yaqa6-libxfont_2-2.0.9' '/nix/store/qhw3400rrgalqgpa8ilc47sn38ml2jk1-libxshmfence-1.3.3-dev' '/nix/store/qlagp5jfymn4mnmqq938cyc4fwcwim7z-libxshmfence-1.3.3' '/nix/store/qdz612hkm62xikmm4b4wkcj8pc7y9lm9-libpciaccess-0.19' '/nix/store/00vmp292cjgwhzp089cszl51xiq8ncrj-systemd-minimal-libs-261.2-dev' '/nix/store/4hlgp6r4j5p6b2b9sxw01cba0x3fjxd6-systemd-minimal-libs-261.2' '/nix/store/5w897qjkd3wgx28h8gnzk2zah213g6lf-xorg-server-21.1.24' '/nix/store/81y35hm10qdnb0s9lbfmy12vbcgnnvv3-xvfb-run-1+g87f6705' '/nix/store/yfaj060r9jn5wxlknnnnm22h9p0ahnbm-cargo-sweep-0.8.0' '/nix/store/4qmwq1p9crkyg97sk14q8g5shxb1cf7r-dioxus-cli-0.8.0-alpha.0' '/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129' '/nix/store/v8rz35q5sk5m2wln3h75abnmymags0p1-wasm-bindgen-cli-0.2.127' '/nix/store/cslxcx72gg4lzgnc8nwfbp5gl9hznap0-tailwindcss_4-4.3.3' '/nix/store/6dhcgiidbg7ldz861yl8jgcj9ax8liyi-cargo-hakari-0.9.38' '/nix/store/z4czsax3mdyxx77mwb0yjarnzb1rip00-just-1.58.0' '/nix/store/rag8h8gpcmksr927bc4y5bgfsb879kmr-prek-0.5.7' '/nix/store/2cy8618y6n260ww0j77gbd6cz77y1har-cargo-rail-0.32.0' '/nix/store/7fx93fz9c809w8yjxb0934zw6xcjis58-pkg-config-wrapper-0.29.2' '/nix/store/4qlkb64qf364lsygwadiiyjjbc0hcq6i-mold-unwrapped-wrapper-2.42.0' '/nix/store/fpig164lz7fcipbndn61sfd2y2yq363n-patchelf-0.15.2' '/nix/store/7n9ldpw06qbqlnf0iaig26agpn2aazfl-update-autotools-gnu-config-scripts-hook' '/nix/store/0y5xmdb7qfvimjwbq7ibg1xdgkgjwqng-no-broken-symlinks.sh' '/nix/store/cv1d7p48379km6a85h4zp6kr86brh32q-audit-tmpdir.sh' '/nix/store/85clx3b0xkdf58jn161iy80y5223ilbi-compress-man-pages.sh' '/nix/store/p3l1a5y7nllfyrjn2krlwgcc3z0cd3fq-make-symlinks-relative.sh' '/nix/store/5yzw0vhkyszf2d179m0qfkgxmp5wjjx4-move-docs.sh' '/nix/store/fyaryjvghbkpfnsyw97hb3lyb37s1pd6-move-lib64.sh' '/nix/store/kd4xwxjpjxi71jkm6ka0np72if9rm3y0-move-sbin.sh' '/nix/store/pag6l61paj1dc9sv15l7bm5c17xn5kyk-move-systemd-user-units.sh' '/nix/store/cmzya9irvxzlkh7lfy6i82gbp0saxqj3-multiple-outputs.sh' '/nix/store/x8c40nfigps493a07sdr2pm5s9j1cdc0-patch-shebangs.sh' '/nix/store/cickvswrvann041nqxb0rxilc46svw1n-prune-libtool-files.sh' '/nix/store/xyff06pkhki3qy1ls77w10s0v79c9il0-reproducible-builds.sh' '/nix/store/z7k98578dfzi6l3hsvbivzm7hfqlk0zc-set-source-date-epoch-to-latest.sh' '/nix/store/89igyzp72y4ky0qyj4x5r52z84w203ki-strip.sh' )
postHook='
populateBindgenEnv
'
preConfigurePhases=' updateAutotoolsGnuConfigScriptsPhase'
outputLib='out'
patches=''
export patches
NIX_BINTOOLS_WRAPPER_TARGET_BUILD_x86_64_unknown_linux_gnu='1'
export NIX_BINTOOLS_WRAPPER_TARGET_BUILD_x86_64_unknown_linux_gnu
GSETTINGS_SCHEMAS_PATH='/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/share/gsettings-schemas/pipewire-1.6.8:/nix/store/w7z6y1f2gnm6qx8gpkxzvbnxsw3b3svl-gsettings-desktop-schemas-50.1/share/gsettings-schemas/gsettings-desktop-schemas-50.1:/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/share/gsettings-schemas/gtk+3-3.24.52'
export GSETTINGS_SCHEMAS_PATH
HOSTTYPE='x86_64'
HOST_PATH='/nix/store/3qgy8q2j64v2m9jy3a5jmssacbblhd4r-coreutils-9.11/bin:/nix/store/i4kjidyvl8ciwbcwm75g75sriq3knav0-findutils-4.11.0/bin:/nix/store/xw09m9cnc9ayb2j53y2rj8jhm1h7kkxa-diffutils-3.12/bin:/nix/store/4nylac9gazdpsid79qyk0jpqm312b2jx-gnused-4.10/bin:/nix/store/yhr4ps9rdmb55pgqaq246vlh7xfac1dx-gnugrep-3.12/bin:/nix/store/bpy2ps3f0f3gvgvqjwgqriw1j1n840wd-gawk-5.4.1/bin:/nix/store/w2mbg6mcq7k3j088sbwfziy8vsdhyk7v-gnutar-1.35/bin:/nix/store/sd9qcv5s2gxvd6hpp5hk5cixq2bd4jd1-gzip-1.14/bin:/nix/store/622jkvg8hgffly7dca6iif9drvimingx-bzip2-1.0.8-bin/bin:/nix/store/hd36xbvbc0vrn9f9idrfwxznri0ax427-gnumake-4.4.1/bin:/nix/store/9ipfvwnqp1q8ijnmi5sxvlx9r8w34lw3-bash-5.3p15/bin:/nix/store/1rcr9yyj9qlgm3dyin1j37xl7q84h15i-patch-2.8/bin:/nix/store/0fb3jrf20z28598alm6g7rrsw6dz2cqj-xz-5.8.3-bin/bin:/nix/store/xcab8m6wdrd30l10bw3hkz32yiibynzv-file-5.48/bin'
export HOST_PATH
PATH='/nix/store/w88q44gqd1qg5wmkk7v0h97rpiqvam0l-gcc-wrapper-15.3.0/bin:/nix/store/4yxdxw3kppr2pbic95p4mri520rkdag2-gcc-15.3.0/bin:/nix/store/axmdd5wlxg1c9dbblvj09pjhkawqxmqx-glibc-2.42-84-bin/bin:/nix/store/3qgy8q2j64v2m9jy3a5jmssacbblhd4r-coreutils-9.11/bin:/nix/store/rm982p6i9ry7rwq5jn638kdpv0a2331f-binutils-wrapper-2.46/bin:/nix/store/5pa1282x1sasbwhmnah6bny1g9pc2lj1-binutils-2.46/bin:/nix/store/4pgr1yxmpsp0dsfvsakaf0cyyv2zhrqx-fleet-doctor/bin:/nix/store/7a60q5dgnv6z96c279rc1nalyiw4mgqn-bash-interactive-5.3p15/bin:/nix/store/50x4m6r9zc0j2iz5cr6ppsac04z057zf-rust-default-1.99.1/bin:/nix/store/2y91v9adfb6kyrwzkjix0j9yx78xmyy0-clang-tools-21.1.8/bin:/nix/store/7ydf0dzbsq7w35bxifci2mslm76pw1qf-gnumake-4.4.1/bin:/nix/store/6jh160z5zby7p11nk8x7z3jdnxmimpjp-ccls-0.20250815.1/bin:/nix/store/bg2bp1zmii5spb2hkgv5xn6mzva7nx9r-gdb-17.2/bin:/nix/store/7p6qjr2kqkbd0i3ylndwrxsb0j7m317k-valgrind-3.27.1/bin:/nix/store/p6s1f49k1s63r79bmqfvynsbbbrqvnca-cargo-watch-8.5.3/bin:/nix/store/wfk7hizfxni5fyaqg3x5pwnpy0qf5d13-cargo-nextest-0.9.143/bin:/nix/store/1gw6in1ws9y9vlh0frhxzixm3k7y7v08-bacon-3.25.0/bin:/nix/store/l6ww8kbj9g5nflwp1m04l7in832z9zsw-flac-1.5.0-bin/bin:/nix/store/cbj4qx4v111j2pw218s1sffl0lsijpb9-uv-0.12.11/bin:/nix/store/bpw06lnk816nznyc9gwmpia5mk5fs5ac-cargo-nightly/bin:/nix/store/63b5kdhvzs4sq7xgg8bikkvdv3kf7c7p-openssl-3.6.3-bin/bin:/nix/store/z203ci88037mkq16c1kn8gy518m3ab1y-freetype-2.14.3-dev/bin:/nix/store/622jkvg8hgffly7dca6iif9drvimingx-bzip2-1.0.8-bin/bin:/nix/store/34vidkz6jm5pinhhyxi64pj82x70vida-brotli-1.2.0/bin:/nix/store/p8m024sxqwg7il6660zqgshrqpsvf43l-libpng-apng-1.6.58-dev/bin:/nix/store/iss09vixlpbq7jwf2y5wkmxvcfydgm3v-expat-2.8.3-dev/bin:/nix/store/122fcxcqy3ay3jkm3fa2bav0vb8mgdjy-fontconfig-2.18.2-bin/bin:/nix/store/908kvhj55s5z7laavf4pddxrxl62rrgl-cmake-4.3.4/bin:/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/bin:/nix/store/8yjpgadavqdwphcd2kw4dfvxmgdp5sb3-git-2.55.0/bin:/nix/store/gvgz0nm1mwj50k0xd2pr78bx9m5mjxvj-alsa-lib-1.2.16.1/bin:/nix/store/bqs4pjxbw1jp2gq48m33myy9iq19m0ws-onnxruntime-1.27.1/bin:/nix/store/ihhpc5fqgwym99dadasw034k9qq76k9s-pipewire-1.6.8/bin:/nix/store/nxxysnlnrgvg42h8fz9zlmsdiixmw4zj-glib-2.88.3-dev/bin:/nix/store/518bcx82g8m9cyji47sprx9ajmadjk8s-gettext-1.0/bin:/nix/store/qyknva7lkacxsgpdpgcqy5kxshwwlp2x-glib-2.88.3-bin/bin:/nix/store/z1pq2ydlcgz8v37m2xdi8ja562b7y7nz-gtk+3-3.24.52-dev/bin:/nix/store/yn8y98bfwgr9jxcrl1x32h04mshpn9wc-dbus-1.16.2-lib/bin:/nix/store/2swbxihjql78pnn5kfmhjq5dvp9m744b-dbus-1.16.2/bin:/nix/store/j8aysi64y93f62rxvf3x2i0h1l299h28-cairo-1.18.4-dev/bin:/nix/store/fnwi54n3m48apqgfl6dskyc5zd2bxjp8-fribidi-1.0.16/bin:/nix/store/pqqqjfb1pgggrljcl0q9fk3alm2b0kh7-gdk-pixbuf-2.44.7-dev/bin:/nix/store/7g0vbmal61bq8bw1vvrcjhrps94kikna-libjpeg-turbo-3.1.4.1-bin/bin:/nix/store/npsbrz5sr41nk35fap059a8ps05wjj63-zstd-1.5.7-bin/bin:/nix/store/xcwclh5qgw2x6qc39x6ixara06xczzlw-zstd-1.5.7/bin:/nix/store/2f0xdvgl9csjsc2yhnfpgm6qyzcypwxl-libdeflate-1.25/bin:/nix/store/0fb3jrf20z28598alm6g7rrsw6dz2cqj-xz-5.8.3-bin/bin:/nix/store/bwaay526pyr0wf3wksskivgxzriyrmld-libwebp-1.6.0/bin:/nix/store/lmxwk264y51a47h1h5zk3phfsqqzw9cz-libtiff-4.7.2-bin/bin:/nix/store/9mlc8y3ykj14p4nwadw93g4s8zgw4skn-gdk-pixbuf-2.44.7/bin:/nix/store/gcvhp35cil7kncli38xlgbk5zy6d7j07-harfbuzz-13.2.1-dev/bin:/nix/store/r3k50gmygh9r5jbdz2a402aszav2zd7d-graphite2-1.3.15/bin:/nix/store/h81cqwzp08wcmnqimammsfqxnf0ghk5f-pango-1.57.1-bin/bin:/nix/store/fsfrc92iamihmgj1ar6zxd2inynp2l66-cups-2.4.19-dev/bin:/nix/store/ns769gwc5z8a1vlkiyz3s1c77g97yksc-cups-2.4.19/bin:/nix/store/fl089m700c7zxqwgly18jyx0fs4lgbbj-gtk+3-3.24.52/bin:/nix/store/f8dxgv2x140iwmzisq03lbgl74jvhqc7-webkitgtk-2.52.6+abi=4.1/bin:/nix/store/skbn0lybxlcsq76s0fz1yb1j966i3fs5-xdotool-4.20260303.1/bin:/nix/store/yk95z4hjlzdk418px0fc797qszccfj5p-libxkbcommon-1.13.2/bin:/nix/store/lz0knivawf4inn00v1lm4vdvg1vx5r2g-pipewire-1.6.8-jack/bin:/nix/store/dniq5gd7rj0jb9g8mk3c7phfpfdxrzci-jack-example-tools-4/bin:/nix/store/5w897qjkd3wgx28h8gnzk2zah213g6lf-xorg-server-21.1.24/bin:/nix/store/81y35hm10qdnb0s9lbfmy12vbcgnnvv3-xvfb-run-1+g87f6705/bin:/nix/store/yfaj060r9jn5wxlknnnnm22h9p0ahnbm-cargo-sweep-0.8.0/bin:/nix/store/4qmwq1p9crkyg97sk14q8g5shxb1cf7r-dioxus-cli-0.8.0-alpha.0/bin:/nix/store/07bfakqii0mdv26wk44k7bv9frprapdv-binaryen-129/bin:/nix/store/v8rz35q5sk5m2wln3h75abnmymags0p1-wasm-bindgen-cli-0.2.127/bin:/nix/store/cslxcx72gg4lzgnc8nwfbp5gl9hznap0-tailwindcss_4-4.3.3/bin:/nix/store/6dhcgiidbg7ldz861yl8jgcj9ax8liyi-cargo-hakari-0.9.38/bin:/nix/store/z4czsax3mdyxx77mwb0yjarnzb1rip00-just-1.58.0/bin:/nix/store/rag8h8gpcmksr927bc4y5bgfsb879kmr-prek-0.5.7/bin:/nix/store/2cy8618y6n260ww0j77gbd6cz77y1har-cargo-rail-0.32.0/bin:/nix/store/7fx93fz9c809w8yjxb0934zw6xcjis58-pkg-config-wrapper-0.29.2/bin:/nix/store/4qlkb64qf364lsygwadiiyjjbc0hcq6i-mold-unwrapped-wrapper-2.42.0/bin:/nix/store/dl5f83ajdgljwnajpdsmnyc7iz0idr9z-mold-unwrapped-2.42.0/bin:/nix/store/fpig164lz7fcipbndn61sfd2y2yq363n-patchelf-0.15.2/bin:/nix/store/3qgy8q2j64v2m9jy3a5jmssacbblhd4r-coreutils-9.11/bin:/nix/store/i4kjidyvl8ciwbcwm75g75sriq3knav0-findutils-4.11.0/bin:/nix/store/xw09m9cnc9ayb2j53y2rj8jhm1h7kkxa-diffutils-3.12/bin:/nix/store/4nylac9gazdpsid79qyk0jpqm312b2jx-gnused-4.10/bin:/nix/store/yhr4ps9rdmb55pgqaq246vlh7xfac1dx-gnugrep-3.12/bin:/nix/store/bpy2ps3f0f3gvgvqjwgqriw1j1n840wd-gawk-5.4.1/bin:/nix/store/w2mbg6mcq7k3j088sbwfziy8vsdhyk7v-gnutar-1.35/bin:/nix/store/sd9qcv5s2gxvd6hpp5hk5cixq2bd4jd1-gzip-1.14/bin:/nix/store/622jkvg8hgffly7dca6iif9drvimingx-bzip2-1.0.8-bin/bin:/nix/store/hd36xbvbc0vrn9f9idrfwxznri0ax427-gnumake-4.4.1/bin:/nix/store/9ipfvwnqp1q8ijnmi5sxvlx9r8w34lw3-bash-5.3p15/bin:/nix/store/1rcr9yyj9qlgm3dyin1j37xl7q84h15i-patch-2.8/bin:/nix/store/0fb3jrf20z28598alm6g7rrsw6dz2cqj-xz-5.8.3-bin/bin:/nix/store/xcab8m6wdrd30l10bw3hkz32yiibynzv-file-5.48/bin'
export PATH
declare -a postFixupHooks=('cmakePcfileCheckPhase' 'noBrokenSymlinksInAllOutputs' '_makeSymlinksRelative' '_multioutPropagateDev' )
installPhase='justInstallPhase'
isystem_seen=''
defaultNativeBuildInputs='/nix/store/fpig164lz7fcipbndn61sfd2y2yq363n-patchelf-0.15.2 /nix/store/7n9ldpw06qbqlnf0iaig26agpn2aazfl-update-autotools-gnu-config-scripts-hook /nix/store/0y5xmdb7qfvimjwbq7ibg1xdgkgjwqng-no-broken-symlinks.sh /nix/store/cv1d7p48379km6a85h4zp6kr86brh32q-audit-tmpdir.sh /nix/store/85clx3b0xkdf58jn161iy80y5223ilbi-compress-man-pages.sh /nix/store/p3l1a5y7nllfyrjn2krlwgcc3z0cd3fq-make-symlinks-relative.sh /nix/store/5yzw0vhkyszf2d179m0qfkgxmp5wjjx4-move-docs.sh /nix/store/fyaryjvghbkpfnsyw97hb3lyb37s1pd6-move-lib64.sh /nix/store/kd4xwxjpjxi71jkm6ka0np72if9rm3y0-move-sbin.sh /nix/store/pag6l61paj1dc9sv15l7bm5c17xn5kyk-move-systemd-user-units.sh /nix/store/cmzya9irvxzlkh7lfy6i82gbp0saxqj3-multiple-outputs.sh /nix/store/x8c40nfigps493a07sdr2pm5s9j1cdc0-patch-shebangs.sh /nix/store/cickvswrvann041nqxb0rxilc46svw1n-prune-libtool-files.sh /nix/store/xyff06pkhki3qy1ls77w10s0v79c9il0-reproducible-builds.sh /nix/store/z7k98578dfzi6l3hsvbivzm7hfqlk0zc-set-source-date-epoch-to-latest.sh /nix/store/89igyzp72y4ky0qyj4x5r52z84w203ki-strip.sh /nix/store/w88q44gqd1qg5wmkk7v0h97rpiqvam0l-gcc-wrapper-15.3.0'
CFLAGS_wasm32_unknown_unknown='-isystem /nix/store/dj85h4lis6cnsxq4iz2k195bnb3g18si-clang-wrapper-18.1.8/resource-root/include'
export CFLAGS_wasm32_unknown_unknown
doInstallCheck=''
export doInstallCheck
declare -a envBuildTargetHooks=('ccWrapper_addCVars' 'bintoolsWrapper_addLDVars' 'addPythonPath' 'sysconfigdataHook' 'gettextDataDirsHook' )
PLAYWRIGHT_BROWSERS_PATH='/nix/store/czm4zg0x2l1735f3zhlz45nzygxkyhi1-playwright-browsers'
export PLAYWRIGHT_BROWSERS_PATH
OPTERR='1'
NIX_BINTOOLS_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu='1'
export NIX_BINTOOLS_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu
WEBKIT_DISABLE_COMPOSITING_MODE='1'
export WEBKIT_DISABLE_COMPOSITING_MODE
OBJDUMP_FOR_BUILD='objdump'
export OBJDUMP_FOR_BUILD
configurePhase ()
{
 
    runHook preConfigure;
    : "${configureScript=}";
    if [[ -z "$configureScript" && -x ./configure ]]; then
        configureScript=./configure;
    fi;
    if [ -z "${dontFixLibtool:-}" ]; then
        export lt_cv_deplibs_check_method="${lt_cv_deplibs_check_method-pass_all}";
        local i;
        find . -iname "ltmain.sh" -print0 | while IFS='' read -r -d '' i; do
            echo "fixing libtool script $i";
            fixLibtool "$i";
        done;
        CONFIGURE_MTIME_REFERENCE=$(mktemp configure.mtime.reference.XXXXXX);
        find . -executable -type f -name configure -exec grep -l 'GNU Libtool is free software; you can redistribute it and/or modify' {} \; -exec touch -r {} "$CONFIGURE_MTIME_REFERENCE" \; -exec sed -i s_/usr/bin/file_file_g {} \; -exec touch -r "$CONFIGURE_MTIME_REFERENCE" {} \;;
        rm -f "$CONFIGURE_MTIME_REFERENCE";
    fi;
    if [[ -z "${dontAddPrefix:-}" && -n "$prefix" ]]; then
        local -r prefixKeyOrDefault="${prefixKey:---prefix=}";
        if [ "${prefixKeyOrDefault: -1}" = " " ]; then
            prependToVar configureFlags "$prefix";
            prependToVar configureFlags "${prefixKeyOrDefault::-1}";
        else
            prependToVar configureFlags "$prefixKeyOrDefault$prefix";
        fi;
    fi;
    if [[ -f "$configureScript" ]]; then
        if [ -z "${dontAddDisableDepTrack:-}" ]; then
            if grep -q dependency-tracking "$configureScript"; then
                prependToVar configureFlags --disable-dependency-tracking;
            fi;
        fi;
        if [ -z "${dontDisableStatic:-}" ]; then
            if grep -q enable-static "$configureScript"; then
                prependToVar configureFlags --disable-static;
            fi;
        fi;
        if [ -z "${dontPatchShebangsInConfigure:-}" ]; then
            patchShebangs --build "$configureScript";
        fi;
    fi;
    if [ -n "$configureScript" ]; then
        local -a flagsArray;
        concatTo flagsArray configureFlags configureFlagsArray;
        echoCmd 'configure flags' "${flagsArray[@]}";
        $configureScript "${flagsArray[@]}";
        unset flagsArray;
    else
        echo "no configure script, doing nothing";
    fi;
    runHook postConfigure
}
dumpVars ()
{
 
    if [[ "${noDumpEnvVars:-0}" != 1 && -d "$NIX_BUILD_TOP" ]]; then
        local old_umask;
        old_umask=$(umask);
        umask 0077;
        export 2> /dev/null > "$NIX_BUILD_TOP/env-vars";
        umask "$old_umask";
    fi
}
stripDirs ()
{
 
    local cmd="$1";
    local ranlibCmd="$2";
    local paths="$3";
    local stripFlags="$4";
    local excludeFlags=();
    local pathsNew=;
    [ -z "$cmd" ] && echo "stripDirs: Strip command is empty" 1>&2 && exit 1;
    [ -z "$ranlibCmd" ] && echo "stripDirs: Ranlib command is empty" 1>&2 && exit 1;
    local pattern;
    if [ -n "${stripExclude:-}" ]; then
        for pattern in "${stripExclude[@]}";
        do
            excludeFlags+=(-a '!' '(' -name "$pattern" -o -wholename "$prefix/$pattern" ')');
        done;
    fi;
    local p;
    for p in ${paths};
    do
        if [ -e "$prefix/$p" ]; then
            pathsNew="${pathsNew} $prefix/$p";
        fi;
    done;
    paths=${pathsNew};
    if [ -n "${paths}" ]; then
        echo "stripping (with command $cmd and flags $stripFlags) in$paths";
        local striperr;
        striperr="$(mktemp --tmpdir="$TMPDIR" 'striperr.XXXXXX')";
        find $paths -type f "${excludeFlags[@]}" -a '!' -path "$prefix/lib/debug/*" -printf '%D-%i,%p\0' | sort -t, -k1,1 -u -z | cut -d, -f2- -z | xargs -r -0 -n1 -P "$NIX_BUILD_CORES" -- $cmd $stripFlags 2> "$striperr" || exit_code=$?;
        [[ "$exit_code" = 123 || -z "$exit_code" ]] || ( cat "$striperr" 1>&2 && exit 1 );
        rm "$striperr";
        find $paths -name '*.a' -type f -exec $ranlibCmd '{}' \; 2> /dev/null;
    fi
}
_moveLib64 ()
{
 
    if [ "${dontMoveLib64-}" = 1 ]; then
        return;
    fi;
    if [ ! -e "$prefix/lib64" -o -L "$prefix/lib64" ]; then
        return;
    fi;
    echo "moving $prefix/lib64/* to $prefix/lib";
    mkdir -p $prefix/lib;
    shopt -s dotglob;
    for i in $prefix/lib64/*;
    do
        mv --no-clobber "$i" $prefix/lib;
    done;
    shopt -u dotglob;
    rmdir $prefix/lib64;
    ln -s lib $prefix/lib64
}
concatStringsSep ()
{
 
    local sep="$1";
    local name="$2";
    local type oldifs;
    if type=$(declare -p "$name" 2> /dev/null); then
        local -n nameref="$name";
        case "${type#* }" in 
            -A*)
                echo "concatStringsSep(): ERROR: trying to use concatStringsSep on an associative array." 1>&2;
                return 1
            ;;
            -a*)
                local IFS="$(printf '\036')"
            ;;
            *)
                local IFS=" "
            ;;
        esac;
        local ifs_separated="${nameref[*]}";
        echo -n "${ifs_separated//"$IFS"/"$sep"}";
    fi
}
mapOffset ()
{
 
    local -r inputOffset="$1";
    local -n outputOffset="$2";
    if (( inputOffset <= 0 )); then
        outputOffset=$((inputOffset + hostOffset));
    else
        outputOffset=$((inputOffset - 1 + targetOffset));
    fi
}
substituteStream ()
{
 
    local var=$1;
    local description=$2;
    shift 2;
    while (( "$#" )); do
        local replace_mode="$1";
        case "$1" in 
            --replace)
                if ! "$_substituteStream_has_warned_replace_deprecation"; then
                    echo "substituteStream() in derivation $name: WARNING: '--replace' is deprecated, use --replace-{fail,warn,quiet}. ($description)" 1>&2;
                    _substituteStream_has_warned_replace_deprecation=true;
                fi;
                replace_mode='--replace-warn'
            ;&
            --replace-quiet | --replace-warn | --replace-fail)
                pattern="$2";
                replacement="$3";
                shift 3;
                if ! [[ "${!var}" == *"$pattern"* ]]; then
                    if [ "$replace_mode" == --replace-warn ]; then
                        printf "substituteStream() in derivation $name: WARNING: pattern %q doesn't match anything in %s\n" "$pattern" "$description" 1>&2;
                    else
                        if [ "$replace_mode" == --replace-fail ]; then
                            printf "substituteStream() in derivation $name: ERROR: pattern %q doesn't match anything in %s\n" "$pattern" "$description" 1>&2;
                            return 1;
                        fi;
                    fi;
                fi;
                eval "$var"'=${'"$var"'//"$pattern"/"$replacement"}'
            ;;
            --subst-var)
                local varName="$2";
                shift 2;
                if ! [[ "$varName" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]; then
                    echo "substituteStream() in derivation $name: ERROR: substitution variables must be valid Bash names, \"$varName\" isn't." 1>&2;
                    return 1;
                fi;
                if [ -z ${!varName+x} ]; then
                    echo "substituteStream() in derivation $name: ERROR: variable \$$varName is unset" 1>&2;
                    return 1;
                fi;
                pattern="@$varName@";
                replacement="${!varName}";
                eval "$var"'=${'"$var"'//"$pattern"/"$replacement"}'
            ;;
            --subst-var-by)
                pattern="@$2@";
                replacement="$3";
                eval "$var"'=${'"$var"'//"$pattern"/"$replacement"}';
                shift 3
            ;;
            *)
                echo "substituteStream() in derivation $name: ERROR: Invalid command line argument: $1" 1>&2;
                return 1
            ;;
        esac;
    done;
    printf "%s" "${!var}"
}
addPythonPath ()
{
 
    addToSearchPathWithCustomDelimiter : PYTHONPATH $1/lib/python3.14/site-packages
}
substituteAllStream ()
{
 
    local -a args=();
    _allFlags;
    substituteStream "$1" "$2" "${args[@]}"
}
activatePackage ()
{
 
    local pkg="$1";
    local -r hostOffset="$2";
    local -r targetOffset="$3";
    (( hostOffset <= targetOffset )) || exit 1;
    if [ -f "$pkg" ]; then
        nixTalkativeLog "sourcing setup hook '$pkg'";
        source "$pkg";
    fi;
    if [[ -z "${strictDeps-}" || "$hostOffset" -le -1 ]]; then
        addToSearchPath _PATH "$pkg/bin";
    fi;
    if (( hostOffset <= -1 )); then
        addToSearchPath _XDG_DATA_DIRS "$pkg/share";
    fi;
    if [[ "$hostOffset" -eq 0 && -d "$pkg/bin" ]]; then
        addToSearchPath _HOST_PATH "$pkg/bin";
    fi;
    if [[ -f "$pkg/nix-support/setup-hook" ]]; then
        nixTalkativeLog "sourcing setup hook '$pkg/nix-support/setup-hook'";
        source "$pkg/nix-support/setup-hook";
    fi
}
genericBuild ()
{
 
    export GZIP_NO_TIMESTAMPS=1;
    if [ -f "${buildCommandPath:-}" ]; then
        source "$buildCommandPath";
        return;
    fi;
    if [ -n "${buildCommand:-}" ]; then
        eval "$buildCommand";
        return;
    fi;
    definePhases;
    for curPhase in ${phases[*]};
    do
        runPhase "$curPhase";
    done
}
addToSearchPathWithCustomDelimiter ()
{
 
    local delimiter="$1";
    local varName="$2";
    local dir="$3";
    if [[ -d "$dir" && "${!varName:+${delimiter}${!varName}${delimiter}}" != *"${delimiter}${dir}${delimiter}"* ]]; then
        export "${varName}=${!varName:+${!varName}${delimiter}}${dir}";
    fi
}
stripHash ()
{
 
    local strippedName casematchOpt=0;
    strippedName="$(basename -- "$1")";
    shopt -q nocasematch && casematchOpt=1;
    shopt -u nocasematch;
    if [[ "$strippedName" =~ ^[a-z0-9]{32}- ]]; then
        echo "${strippedName:33}";
    else
        echo "$strippedName";
    fi;
    if (( casematchOpt )); then
        shopt -s nocasematch;
    fi
}
nixWarnLog ()
{
 
    _nixLogWithLevel 1 "$*"
}
_makeSymlinksRelative ()
{
 
    local prefixes;
    prefixes=();
    for output in $(getAllOutputNames);
    do
        [ ! -e "${!output}" ] && continue;
        prefixes+=("${!output}");
    done;
    find "${prefixes[@]}" -type l -printf '%H\0%p\0' | xargs -0 -n2 -r -P "$NIX_BUILD_CORES" sh -c '
      output="$1"
      link="$2"

      linkTarget=$(readlink "$link")

      # only touch links that point inside the same output tree
      [[ $linkTarget == "$output"/* ]] || exit 0

      if [ ! -e "$linkTarget" ]; then
        echo "the symlink $link is broken, it points to $linkTarget (which is missing)"
      fi

      echo "making symlink relative: $link"
      ln -snrf "$linkTarget" "$link"
    ' _
}
_logHook ()
{
 
    if [[ -z ${NIX_LOG_FD-} ]]; then
        return;
    fi;
    local hookKind="$1";
    local hookExpr="$2";
    shift 2;
    if declare -F "$hookExpr" > /dev/null 2>&1; then
        nixTalkativeLog "calling '$hookKind' function hook '$hookExpr'" "$@";
    else
        if type -p "$hookExpr" > /dev/null; then
            nixTalkativeLog "sourcing '$hookKind' script hook '$hookExpr'";
        else
            if [[ "$hookExpr" != "_callImplicitHook"* ]]; then
                local exprToOutput;
                if [[ ${NIX_DEBUG:-0} -ge 5 ]]; then
                    exprToOutput="$hookExpr";
                else
                    local hookExprLine;
                    while IFS= read -r hookExprLine; do
                        hookExprLine="${hookExprLine#"${hookExprLine%%[![:space:]]*}"}";
                        if [[ -n "$hookExprLine" ]]; then
                            exprToOutput+="$hookExprLine\\n ";
                        fi;
                    done <<< "$hookExpr";
                    exprToOutput="${exprToOutput%%\\n }";
                fi;
                nixTalkativeLog "evaling '$hookKind' string hook '$exprToOutput'";
            fi;
        fi;
    fi
}
makeCmakeFindLibs ()
{
 
    isystem_seen=;
    iframework_seen=;
    for flag in ${NIX_CFLAGS_COMPILE-} ${NIX_LDFLAGS-};
    do
        if test -n "$isystem_seen" && test -d "$flag"; then
            isystem_seen=;
            addToSearchPath CMAKE_INCLUDE_PATH "${flag}";
        else
            if test -n "$iframework_seen" && test -d "$flag"; then
                iframework_seen=;
                addToSearchPath CMAKE_FRAMEWORK_PATH "${flag}";
            else
                isystem_seen=;
                iframework_seen=;
                case $flag in 
                    -I*)
                        addToSearchPath CMAKE_INCLUDE_PATH "${flag:2}"
                    ;;
                    -L*)
                        addToSearchPath CMAKE_LIBRARY_PATH "${flag:2}"
                    ;;
                    -F*)
                        addToSearchPath CMAKE_FRAMEWORK_PATH "${flag:2}"
                    ;;
                    -isystem)
                        isystem_seen=1
                    ;;
                    -iframework)
                        iframework_seen=1
                    ;;
                esac;
            fi;
        fi;
    done
}
getHostRole ()
{
 
    getRole "$hostOffset"
}
fixLibtool ()
{
 
    local search_path;
    for flag in $NIX_LDFLAGS;
    do
        case $flag in 
            -L*)
                search_path+=" ${flag#-L}"
            ;;
        esac;
    done;
    sed -i "$1" -e "s^eval \(sys_lib_search_path=\).*^\1'${search_path:-}'^" -e 's^eval sys_lib_.+search_path=.*^^'
}
unpackPhase ()
{
 
    runHook preUnpack;
    if [ -z "${srcs:-}" ]; then
        if [ -z "${src:-}" ]; then
            echo 'variable $src or $srcs should point to the source';
            exit 1;
        fi;
        srcs="$src";
    fi;
    local -a srcsArray;
    concatTo srcsArray srcs;
    local dirsBefore="";
    for i in *;
    do
        if [ -d "$i" ]; then
            dirsBefore="$dirsBefore $i ";
        fi;
    done;
    for i in "${srcsArray[@]}";
    do
        unpackFile "$i";
    done;
    : "${sourceRoot=}";
    if [ -n "${setSourceRoot:-}" ]; then
        runOneHook setSourceRoot;
    else
        if [ -z "$sourceRoot" ]; then
            for i in *;
            do
                if [ -d "$i" ]; then
                    case $dirsBefore in 
                        *\ $i\ *)

                        ;;
                        *)
                            if [ -n "$sourceRoot" ]; then
                                echo "unpacker produced multiple directories";
                                exit 1;
                            fi;
                            sourceRoot="$i"
                        ;;
                    esac;
                fi;
            done;
        fi;
    fi;
    if [ -z "$sourceRoot" ]; then
        echo "unpacker appears to have produced no directories";
        exit 1;
    fi;
    echo "source root is $sourceRoot";
    if [ "${dontMakeSourcesWritable:-0}" != 1 ]; then
        chmod -R u+w -- "$sourceRoot";
    fi;
    runHook postUnpack
}
substitute ()
{
 
    local input="$1";
    local output="$2";
    shift 2;
    if [ ! -f "$input" ]; then
        echo "substitute(): ERROR: file '$input' does not exist" 1>&2;
        return 1;
    fi;
    local content;
    consumeEntire content < "$input";
    if [ -e "$output" ]; then
        chmod +w "$output";
    fi;
    substituteStream content "file '$input'" "$@" > "$output"
}
showPhaseHeader ()
{
 
    local phase="$1";
    echo "Running phase: $phase";
    if [[ -z ${NIX_LOG_FD-} ]]; then
        return;
    fi;
    printf "@nix { \"action\": \"setPhase\", \"phase\": \"%s\" }\n" "$phase" >&"$NIX_LOG_FD"
}
patchShebangs ()
{
 
    local pathName;
    local update=false;
    while [[ $# -gt 0 ]]; do
        case "$1" in 
            --host)
                pathName=HOST_PATH;
                shift
            ;;
            --build)
                pathName=PATH;
                shift
            ;;
            --update)
                update=true;
                shift
            ;;
            --)
                shift;
                break
            ;;
            -* | --*)
                echo "Unknown option $1 supplied to patchShebangs" 1>&2;
                return 1
            ;;
            *)
                break
            ;;
        esac;
    done;
    echo "patching script interpreter paths in $@";
    local f;
    local oldPath;
    local newPath;
    local arg0;
    local args;
    local oldInterpreterLine;
    local newInterpreterLine;
    if [[ $# -eq 0 ]]; then
        echo "No arguments supplied to patchShebangs" 1>&2;
        return 0;
    fi;
    local f;
    while IFS= read -r -d '' f; do
        isScript "$f" || continue;
        read -r oldInterpreterLine < "$f" || [ "$oldInterpreterLine" ];
        read -r oldPath arg0 args <<< "${oldInterpreterLine:2}";
        if [[ -z "${pathName:-}" ]]; then
            if [[ -n $strictDeps && $f == "$NIX_STORE"* ]]; then
                pathName=HOST_PATH;
            else
                pathName=PATH;
            fi;
        fi;
        if [[ "$oldPath" == *"/bin/env" ]]; then
            if [[ $arg0 == "-S" ]]; then
                arg0=${args%% *};
                [[ "$args" == *" "* ]] && args=${args#* } || args=;
                newPath="$(PATH="${!pathName}" type -P "env" || true)";
                args="-S $(PATH="${!pathName}" type -P "$arg0" || true) $args";
            else
                if [[ $arg0 == "-"* || $arg0 == *"="* ]]; then
                    echo "$f: unsupported interpreter directive \"$oldInterpreterLine\" (set dontPatchShebangs=1 and handle shebang patching yourself)" 1>&2;
                    exit 1;
                else
                    newPath="$(PATH="${!pathName}" type -P "$arg0" || true)";
                fi;
            fi;
        else
            if [[ -z $oldPath ]]; then
                oldPath="/bin/sh";
            fi;
            newPath="$(PATH="${!pathName}" type -P "$(basename "$oldPath")" || true)";
            args="$arg0 $args";
        fi;
        newInterpreterLine="$newPath $args";
        newInterpreterLine=${newInterpreterLine%${newInterpreterLine##*[![:space:]]}};
        if [[ -n "$oldPath" && ( "$update" == true || "${oldPath:0:${#NIX_STORE}}" != "$NIX_STORE" ) ]]; then
            if [[ -n "$newPath" && "$newPath" != "$oldPath" ]]; then
                echo "$f: interpreter directive changed from \"$oldInterpreterLine\" to \"$newInterpreterLine\"";
                escapedInterpreterLine=${newInterpreterLine//\\/\\\\};
                timestamp=$(stat --printf "%y" "$f");
                tmpFile=$(mktemp -t patchShebangs.XXXXXXXXXX);
                sed -e "1 s|.*|#\!$escapedInterpreterLine|" "$f" > "$tmpFile";
                local restoreReadOnly;
                if [[ ! -w "$f" ]]; then
                    chmod +w "$f";
                    restoreReadOnly=true;
                fi;
                cat "$tmpFile" > "$f";
                rm "$tmpFile";
                if [[ -n "${restoreReadOnly:-}" ]]; then
                    chmod -w "$f";
                fi;
                touch --date "$timestamp" "$f";
            fi;
        fi;
    done < <(find "$@" -type f -perm -0100 -print0)
}
moveToOutput ()
{
 
    local patt="$1";
    local dstOut="$2";
    local output;
    for output in $(getAllOutputNames);
    do
        if [ "${!output}" = "$dstOut" ]; then
            continue;
        fi;
        local srcPath;
        for srcPath in "${!output}"/$patt;
        do
            if [ ! -e "$srcPath" ] && [ ! -L "$srcPath" ]; then
                continue;
            fi;
            if [ "$dstOut" = REMOVE ]; then
                echo "Removing $srcPath";
                rm -r "$srcPath";
            else
                local dstPath="$dstOut${srcPath#${!output}}";
                echo "Moving $srcPath to $dstPath";
                if [ -d "$dstPath" ] && [ -d "$srcPath" ]; then
                    rmdir "$srcPath" --ignore-fail-on-non-empty;
                    if [ -d "$srcPath" ]; then
                        mv -t "$dstPath" "$srcPath"/*;
                        rmdir "$srcPath";
                    fi;
                else
                    mkdir -p "$(readlink -m "$dstPath/..")";
                    mv "$srcPath" "$dstPath";
                fi;
            fi;
            local srcParent="$(readlink -m "$srcPath/..")";
            if [ -n "$(find "$srcParent" -maxdepth 0 -type d -empty 2> /dev/null)" ]; then
                echo "Removing empty $srcParent/ and (possibly) its parents";
                rmdir -p --ignore-fail-on-non-empty "$srcParent" 2> /dev/null || true;
            fi;
        done;
    done
}
printWords ()
{
 
    (( "$#" > 0 )) || return 0;
    printf '%s ' "$@"
}
patchELF ()
{
 
    local dir="$1";
    [ -e "$dir" ] || return 0;
    echo "shrinking RPATHs of ELF executables and libraries in $dir";
    local i;
    while IFS= read -r -d '' i; do
        if [[ "$i" =~ .build-id ]]; then
            continue;
        fi;
        if ! isELF "$i"; then
            continue;
        fi;
        echo "shrinking $i";
        patchelf --shrink-rpath "$i" || true;
    done < <(find "$dir" -type f -print0)
}
nixInfoLog ()
{
 
    _nixLogWithLevel 3 "$*"
}
isELF ()
{
 
    local fn="$1";
    local fd;
    local magic;
    exec {fd}< "$fn";
    LANG=C read -r -n 4 -u "$fd" magic;
    exec {fd}>&-;
    if [ "$magic" = 'ELF' ]; then
        return 0;
    else
        return 1;
    fi
}
_multioutDocs ()
{
 
    local REMOVE=REMOVE;
    moveToOutput share/info "${!outputInfo}";
    moveToOutput share/doc "${!outputDoc}";
    moveToOutput share/gtk-doc "${!outputDevdoc}";
    moveToOutput share/devhelp/books "${!outputDevdoc}";
    moveToOutput share/man "${!outputMan}";
    moveToOutput share/man/man3 "${!outputDevman}"
}
justInstallPhase ()
{
 
    runHook preInstall;
    local flagsArray=();
    concatTo flagsArray justFlags justFlagsArray installTargets=install;
    echoCmd 'install flags' "${flagsArray[@]}";
    just "${flagsArray[@]}";
    runHook postInstall
}
addCMakeParams ()
{
 
    addToSearchPath NIXPKGS_CMAKE_PREFIX_PATH $1
}
distPhase ()
{
 
    runHook preDist;
    local flagsArray=();
    concatTo flagsArray distFlags distFlagsArray distTarget=dist;
    echo 'dist flags: %q' "${flagsArray[@]}";
    make ${makefile:+-f $makefile} "${flagsArray[@]}";
    if [ "${dontCopyDist:-0}" != 1 ]; then
        mkdir -p "$out/tarballs";
        cp -pvd ${tarballs[*]:-*.tar.gz} "$out/tarballs";
    fi;
    runHook postDist
}
patchPhase ()
{
 
    runHook prePatch;
    local -a patchesArray;
    concatTo patchesArray patches;
    local -a flagsArray;
    concatTo flagsArray patchFlags=-p1;
    for i in "${patchesArray[@]}";
    do
        echo "applying patch $i";
        local uncompress=cat;
        case "$i" in 
            *.gz)
                uncompress="gzip -d"
            ;;
            *.bz2)
                uncompress="bzip2 -d"
            ;;
            *.xz)
                uncompress="xz -d"
            ;;
            *.lzma)
                uncompress="lzma -d"
            ;;
        esac;
        $uncompress < "$i" 2>&1 | patch "${flagsArray[@]}";
    done;
    runHook postPatch
}
pkgConfigWrapper_addPkgConfigPath ()
{
 
    local role_post;
    getHostRoleEnvHook;
    addToSearchPath "PKG_CONFIG_PATH${role_post}" "$1/lib/pkgconfig";
    addToSearchPath "PKG_CONFIG_PATH${role_post}" "$1/share/pkgconfig"
}
runHook ()
{
 
    local hookName="$1";
    shift;
    local hooksSlice="${hookName%Hook}Hooks[@]";
    local hook;
    for hook in "_callImplicitHook 0 $hookName" ${!hooksSlice+"${!hooksSlice}"};
    do
        _logHook "$hookName" "$hook" "$@";
        _eval "$hook" "$@";
    done;
    return 0
}
updateSourceDateEpoch ()
{
 
    local path="$1";
    [[ $path == -* ]] && path="./$path";
    local -a res=($(find "$path" -type f -not -newer "$NIX_BUILD_TOP/.." -printf '%T@ "%p"\0' | sort -n --zero-terminated | tail -n1 --zero-terminated | head -c -1));
    local time="${res[0]//\.[0-9]*/}";
    local newestFile="${res[1]}";
    if [ "${time:-0}" -gt "$SOURCE_DATE_EPOCH" ]; then
        echo "setting SOURCE_DATE_EPOCH to timestamp $time of file $newestFile";
        export SOURCE_DATE_EPOCH="$time";
        local now="$(date +%s)";
        if [ "$time" -gt $((now - 60)) ]; then
            echo "warning: file $newestFile may be generated; SOURCE_DATE_EPOCH may be non-deterministic";
        fi;
    fi
}
_multioutPropagateDev ()
{
 
    if [ "$(getAllOutputNames)" = "out" ]; then
        return;
    fi;
    local outputFirst;
    for outputFirst in $(getAllOutputNames);
    do
        break;
    done;
    local propagaterOutput="$outputDev";
    if [ -z "$propagaterOutput" ]; then
        propagaterOutput="$outputFirst";
    fi;
    if [ -z "${propagatedBuildOutputs+1}" ]; then
        local po_dirty="$outputBin $outputInclude $outputLib";
        set +o pipefail;
        propagatedBuildOutputs=`echo "$po_dirty"             | tr -s ' ' '\n' | grep -v -F "$propagaterOutput"             | sort -u | tr '\n' ' ' `;
        set -o pipefail;
    fi;
    if [ -z "$propagatedBuildOutputs" ]; then
        return;
    fi;
    mkdir -p "${!propagaterOutput}"/nix-support;
    for output in $propagatedBuildOutputs;
    do
        echo -n " ${!output}" >> "${!propagaterOutput}"/nix-support/propagated-build-inputs;
    done
}
checkPhase ()
{
 
    runHook preCheck;
    if [[ -z "${foundMakefile:-}" ]]; then
        echo "no Makefile or custom checkPhase, doing nothing";
        runHook postCheck;
        return;
    fi;
    if [[ -z "${checkTarget:-}" ]]; then
        if make -n ${makefile:+-f $makefile} check > /dev/null 2>&1; then
            checkTarget="check";
        else
            if make -n ${makefile:+-f $makefile} test > /dev/null 2>&1; then
                checkTarget="test";
            fi;
        fi;
    fi;
    if [[ -z "${checkTarget:-}" ]]; then
        echo "no check/test target in ${makefile:-Makefile}, doing nothing";
    else
        local flagsArray=(${enableParallelChecking:+-j${NIX_BUILD_CORES}} SHELL="$SHELL");
        concatTo flagsArray makeFlags makeFlagsArray checkFlags=VERBOSE=y checkFlagsArray checkTarget;
        echoCmd 'check flags' "${flagsArray[@]}";
        make ${makefile:+-f $makefile} "${flagsArray[@]}";
        unset flagsArray;
    fi;
    runHook postCheck
}
_eval ()
{
 
    if declare -F "$1" > /dev/null 2>&1; then
        "$@";
    else
        eval "$1";
    fi
}
getRole ()
{
 
    case $1 in 
        -1)
            role_post='_FOR_BUILD'
        ;;
        0)
            role_post=''
        ;;
        1)
            role_post='_FOR_TARGET'
        ;;
        *)
            echo "mold-unwrapped-wrapper-2.42.0: used as improper sort of dependency" 1>&2;
            return 1
        ;;
    esac
}
toPythonPath ()
{
 
    local paths="$1";
    local result=;
    for i in $paths;
    do
        p="$i/lib/python3.14/site-packages";
        result="${result}${result:+:}$p";
    done;
    echo $result
}
nixNoticeLog ()
{
 
    _nixLogWithLevel 2 "$*"
}
definePhases ()
{
 
    if [ -z "${phases[*]:-}" ]; then
        phases="${prePhases[*]:-} unpackPhase patchPhase ${preConfigurePhases[*]:-}             configurePhase ${preBuildPhases[*]:-} buildPhase checkPhase             ${preInstallPhases[*]:-} installPhase ${preFixupPhases[*]:-} fixupPhase installCheckPhase             ${preDistPhases[*]:-} distPhase ${postPhases[*]:-}";
    fi
}
exitHandler ()
{
 
    exitCode="$?";
    set +e;
    if [ -n "${showBuildStats:-}" ]; then
        read -r -d '' -a buildTimes < <(times);
        echo "build times:";
        echo "user time for the shell             ${buildTimes[0]}";
        echo "system time for the shell           ${buildTimes[1]}";
        echo "user time for all child processes   ${buildTimes[2]}";
        echo "system time for all child processes ${buildTimes[3]}";
    fi;
    if (( "$exitCode" != 0 )); then
        runHook failureHook;
        if [ -n "${succeedOnFailure:-}" ]; then
            echo "build failed with exit code $exitCode (ignored)";
            mkdir -p "$out/nix-support";
            printf "%s" "$exitCode" > "$out/nix-support/failed";
            exit 0;
        fi;
    else
        runHook exitHook;
    fi;
    return "$exitCode"
}
addEnvHooks ()
{
 
    local depHostOffset="$1";
    shift;
    local pkgHookVarsSlice="${pkgHookVarVars[$depHostOffset + 1]}[@]";
    local pkgHookVar;
    for pkgHookVar in "${!pkgHookVarsSlice}";
    do
        eval "${pkgHookVar}s"'+=("$@")';
    done
}
nixTalkativeLog ()
{
 
    _nixLogWithLevel 4 "$*"
}
findInputs ()
{
 
    local -r pkg="$1";
    local -r hostOffset="$2";
    local -r targetOffset="$3";
    (( hostOffset <= targetOffset )) || exit 1;
    local varVar="${pkgAccumVarVars[hostOffset + 1]}";
    local varRef="$varVar[$((targetOffset - hostOffset))]";
    local var="${!varRef}";
    unset -v varVar varRef;
    local varSlice="$var[*]";
    case " ${!varSlice-} " in 
        *" $pkg "*)
            return 0
        ;;
    esac;
    unset -v varSlice;
    eval "$var"'+=("$pkg")';
    if ! [ -e "$pkg" ]; then
        echo "build input $pkg does not exist" 1>&2;
        exit 1;
    fi;
    function mapOffset () 
    { 
        local -r inputOffset="$1";
        local -n outputOffset="$2";
        if (( inputOffset <= 0 )); then
            outputOffset=$((inputOffset + hostOffset));
        else
            outputOffset=$((inputOffset - 1 + targetOffset));
        fi
    };
    local relHostOffset;
    for relHostOffset in "${allPlatOffsets[@]}";
    do
        local files="${propagatedDepFilesVars[relHostOffset + 1]}";
        local hostOffsetNext;
        mapOffset "$relHostOffset" hostOffsetNext;
        (( -1 <= hostOffsetNext && hostOffsetNext <= 1 )) || continue;
        local relTargetOffset;
        for relTargetOffset in "${allPlatOffsets[@]}";
        do
            (( "$relHostOffset" <= "$relTargetOffset" )) || continue;
            local fileRef="${files}[$relTargetOffset - $relHostOffset]";
            local file="${!fileRef}";
            unset -v fileRef;
            local targetOffsetNext;
            mapOffset "$relTargetOffset" targetOffsetNext;
            (( -1 <= hostOffsetNext && hostOffsetNext <= 1 )) || continue;
            [[ -f "$pkg/nix-support/$file" ]] || continue;
            local pkgNext;
            read -r -d '' pkgNext < "$pkg/nix-support/$file" || true;
            for pkgNext in $pkgNext;
            do
                findInputs "$pkgNext" "$hostOffsetNext" "$targetOffsetNext";
            done;
        done;
    done
}
bintoolsWrapper_addLDVars ()
{
 
    local role_post;
    getHostRoleEnvHook;
    if [[ -d "$1/lib64" && ! -L "$1/lib64" ]]; then
        export NIX_LDFLAGS${role_post}+=" -L$1/lib64";
    fi;
    if [[ -d "$1/lib" ]]; then
        local -a glob=($1/lib/lib*);
        if [ "${#glob[*]}" -gt 0 ]; then
            export NIX_LDFLAGS${role_post}+=" -L$1/lib";
        fi;
    fi
}
ccWrapper_addCVars ()
{
 
    local role_post;
    getHostRoleEnvHook;
    local found=;
    if [ -d "$1/include" ]; then
        export NIX_CFLAGS_COMPILE${role_post}+=" -isystem $1/include";
        found=1;
    fi;
    if [ -d "$1/Library/Frameworks" ]; then
        export NIX_CFLAGS_COMPILE${role_post}+=" -iframework $1/Library/Frameworks";
        found=1;
    fi;
    if [[ -n "" && -n ${NIX_STORE:-} && -n $found ]]; then
        local scrubbed="$NIX_STORE/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-${1#"$NIX_STORE"/*-}";
        export NIX_CFLAGS_COMPILE${role_post}+=" -fmacro-prefix-map=$1=$scrubbed";
    fi
}
echoCmd ()
{
 
    printf "%s:" "$1";
    shift;
    printf ' %q' "$@";
    echo
}
showPhaseFooter ()
{
 
    local phase="$1";
    local startTime="$2";
    local endTime="$3";
    local delta=$(( endTime - startTime ));
    (( delta < 30 )) && return;
    local H=$((delta/3600));
    local M=$((delta%3600/60));
    local S=$((delta%60));
    echo -n "$phase completed in ";
    (( H > 0 )) && echo -n "$H hours ";
    (( M > 0 )) && echo -n "$M minutes ";
    echo "$S seconds"
}
substituteAllInPlace ()
{
 
    local fileName="$1";
    shift;
    substituteAll "$fileName" "$fileName" "$@"
}
compressManPages ()
{
 
    local dir="$1";
    if [ -L "$dir"/share ] || [ -L "$dir"/share/man ] || [ ! -d "$dir/share/man" ]; then
        return;
    fi;
    echo "gzipping man pages under $dir/share/man/";
    find "$dir"/share/man/ -type f -a '!' -regex '.*\.\(bz2\|gz\|xz\)$' -print0 | xargs -0 -n1 -P "$NIX_BUILD_CORES" gzip -n -f;
    find "$dir"/share/man/ -type l -a '!' -regex '.*\.\(bz2\|gz\|xz\)$' -print0 | sort -z | while IFS= read -r -d '' f; do
        local target;
        target="$(readlink -f "$f")";
        if [ -f "$target".gz ]; then
            ln -sf "$target".gz "$f".gz && rm "$f";
        fi;
    done
}
installCheckPhase ()
{
 
    runHook preInstallCheck;
    if [[ -z "${foundMakefile:-}" ]]; then
        echo "no Makefile or custom installCheckPhase, doing nothing";
    else
        if [[ -z "${installCheckTarget:-}" ]] && ! make -n ${makefile:+-f $makefile} "${installCheckTarget:-installcheck}" > /dev/null 2>&1; then
            echo "no installcheck target in ${makefile:-Makefile}, doing nothing";
        else
            local flagsArray=(${enableParallelChecking:+-j${NIX_BUILD_CORES}} SHELL="$SHELL");
            concatTo flagsArray makeFlags makeFlagsArray installCheckFlags installCheckFlagsArray installCheckTarget=installcheck;
            echoCmd 'installcheck flags' "${flagsArray[@]}";
            make ${makefile:+-f $makefile} "${flagsArray[@]}";
            unset flagsArray;
        fi;
    fi;
    runHook postInstallCheck
}
substituteAll ()
{
 
    local input="$1";
    local output="$2";
    local -a args=();
    _allFlags;
    substitute "$input" "$output" "${args[@]}"
}
_updateSourceDateEpochFromSourceRoot ()
{
 
    if [ -n "$sourceRoot" ]; then
        updateSourceDateEpoch "$sourceRoot";
    fi
}
_overrideFirst ()
{
 
    if [ -z "${!1-}" ]; then
        _assignFirst "$@";
    fi
}
getTargetRoleEnvHook ()
{
 
    getRole "$depTargetOffset"
}
substituteInPlace ()
{
 
    local -a fileNames=();
    for arg in "$@";
    do
        if [[ "$arg" = "--"* ]]; then
            break;
        fi;
        fileNames+=("$arg");
        shift;
    done;
    if ! [[ "${#fileNames[@]}" -gt 0 ]]; then
        echo "substituteInPlace called without any files to operate on (files must come before options!)" 1>&2;
        return 1;
    fi;
    for file in "${fileNames[@]}";
    do
        substitute "$file" "$file" "$@";
    done
}
buildPhase ()
{
 
    runHook preBuild;
    if [[ -z "${makeFlags-}" && -z "${makefile:-}" && ! ( -e Makefile || -e makefile || -e GNUmakefile ) ]]; then
        echo "no Makefile or custom buildPhase, doing nothing";
    else
        foundMakefile=1;
        local flagsArray=(${enableParallelBuilding:+-j${NIX_BUILD_CORES}} SHELL="$SHELL");
        concatTo flagsArray makeFlags makeFlagsArray buildFlags buildFlagsArray;
        echoCmd 'build flags' "${flagsArray[@]}";
        make ${makefile:+-f $makefile} "${flagsArray[@]}";
        unset flagsArray;
    fi;
    runHook postBuild
}
appendToVar ()
{
 
    local -n nameref="$1";
    local useArray type;
    if [ -n "$__structuredAttrs" ]; then
        useArray=true;
    else
        useArray=false;
    fi;
    if type=$(declare -p "$1" 2> /dev/null); then
        case "${type#* }" in 
            -A*)
                echo "appendToVar(): ERROR: trying to use appendToVar on an associative array, use variable+=([\"X\"]=\"Y\") instead." 1>&2;
                return 1
            ;;
            -a*)
                useArray=true
            ;;
            *)
                useArray=false
            ;;
        esac;
    fi;
    shift;
    if $useArray; then
        nameref=(${nameref+"${nameref[@]}"} "$@");
    else
        nameref="${nameref-} $*";
    fi
}
nixChattyLog ()
{
 
    _nixLogWithLevel 5 "$*"
}
cmakePcfileCheckPhase ()
{
 
    while IFS= read -rd '' file; do
        grepout=$(grep --line-number '}//nix/store' "$file" || true);
        if [ -n "$grepout" ]; then
            { 
                echo "Broken paths found in a .pc file! $file";
                echo "The following lines have issues (specifically '//' in paths).";
                echo "$grepout";
                echo "It is very likely that paths are being joined improperly.";
                echo 'ex: "${prefix}/@CMAKE_INSTALL_LIBDIR@" should be "@CMAKE_INSTALL_FULL_LIBDIR@"';
                echo "Please see https://github.com/NixOS/nixpkgs/issues/144170 for more details.";
                exit 1
            } 1>&2;
        fi;
    done < <(find "${!outputDev}" -iname "*.pc" -print0)
}
_defaultUnpack ()
{
 
    local fn="$1";
    local destination;
    if [ -d "$fn" ]; then
        destination="$(stripHash "$fn")";
        if [ -e "$destination" ]; then
            echo "Cannot copy $fn to $destination: destination already exists!";
            echo "Did you specify two \"srcs\" with the same \"name\"?";
            return 1;
        fi;
        cp -r --preserve=timestamps --reflink=auto -- "$fn" "$destination";
    else
        case "$fn" in 
            *.tar.xz | *.tar.lzma | *.txz)
                ( XZ_OPT="--threads=$NIX_BUILD_CORES" xz -d < "$fn";
                true ) | tar xf - --mode=+w --warning=no-timestamp
            ;;
            *.tar | *.tar.* | *.tgz | *.tbz2 | *.tbz)
                tar xf "$fn" --mode=+w --warning=no-timestamp
            ;;
            *)
                return 1
            ;;
        esac;
    fi
}
recordPropagatedDependencies ()
{
 
    declare -ra flatVars=(depsBuildBuildPropagated propagatedNativeBuildInputs depsBuildTargetPropagated depsHostHostPropagated propagatedBuildInputs depsTargetTargetPropagated);
    declare -ra flatFiles=("${propagatedBuildDepFiles[@]}" "${propagatedHostDepFiles[@]}" "${propagatedTargetDepFiles[@]}");
    local propagatedInputsIndex;
    for propagatedInputsIndex in "${!flatVars[@]}";
    do
        local propagatedInputsSlice="${flatVars[$propagatedInputsIndex]}[@]";
        local propagatedInputsFile="${flatFiles[$propagatedInputsIndex]}";
        [[ -n "${!propagatedInputsSlice}" ]] || continue;
        mkdir -p "${!outputDev}/nix-support";
        printWords ${!propagatedInputsSlice} > "${!outputDev}/nix-support/$propagatedInputsFile";
    done
}
sysconfigdataHook ()
{
 
    if [ "$1" = '/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7' ]; then
        export _PYTHON_HOST_PLATFORM='linux-x86_64';
        export _PYTHON_SYSCONFIGDATA_NAME='_sysconfigdata__linux_x86_64-linux-gnu';
    fi
}
printPhases ()
{
 
    definePhases;
    local phase;
    for phase in ${phases[*]};
    do
        printf '%s\n' "$phase";
    done
}
_allFlags ()
{
 
    export system pname name version;
    while IFS='' read -r varName; do
        nixTalkativeLog "@${varName}@ -> ${!varName}";
        args+=("--subst-var" "$varName");
    done < <(awk 'BEGIN { for (v in ENVIRON) if (v ~ /^[a-z][a-zA-Z0-9_]*$/) print v }')
}
_multioutDevs ()
{
 
    if [ "$(getAllOutputNames)" = "out" ] || [ -z "${moveToDev-1}" ]; then
        return;
    fi;
    moveToOutput include "${!outputInclude}";
    moveToOutput lib/pkgconfig "${!outputDev}";
    moveToOutput share/pkgconfig "${!outputDev}";
    moveToOutput lib/cmake "${!outputDev}";
    moveToOutput share/aclocal "${!outputDev}";
    for f in "${!outputDev}"/{lib,share}/pkgconfig/*.pc;
    do
        echo "Patching '$f' includedir to output ${!outputInclude}";
        sed -i "/^includedir=/s,=\${prefix},=${!outputInclude}," "$f";
    done
}
isMachO ()
{
 
    local fn="$1";
    local fd;
    local magic;
    exec {fd}< "$fn";
    LANG=C read -r -n 4 -u "$fd" magic;
    exec {fd}>&-;
    if [[ "$magic" = $(echo -ne "\xfe\xed\xfa\xcf") || "$magic" = $(echo -ne "\xcf\xfa\xed\xfe") ]]; then
        return 0;
    else
        if [[ "$magic" = $(echo -ne "\xfe\xed\xfa\xce") || "$magic" = $(echo -ne "\xce\xfa\xed\xfe") ]]; then
            return 0;
        else
            if [[ "$magic" = $(echo -ne "\xca\xfe\xba\xbe") || "$magic" = $(echo -ne "\xbe\xba\xfe\xca") ]]; then
                return 0;
            else
                return 1;
            fi;
        fi;
    fi
}
cmakeConfigurePhase ()
{
 
    runHook preConfigure;
    : ${cmakeBuildDir:=build};
    export CTEST_OUTPUT_ON_FAILURE=1;
    if [ -n "${enableParallelChecking-1}" ]; then
        export CTEST_PARALLEL_LEVEL=$NIX_BUILD_CORES;
    fi;
    if [ -z "${dontUseCmakeBuildDir-}" ]; then
        mkdir -p "$cmakeBuildDir";
        cd "$cmakeBuildDir";
        : ${cmakeDir:=..};
    else
        : ${cmakeDir:=.};
    fi;
    if [ -z "${dontAddPrefix-}" ]; then
        prependToVar cmakeFlags "-DCMAKE_INSTALL_PREFIX=$prefix";
    fi;
    prependToVar cmakeFlags "-DCMAKE_CXX_COMPILER=$CXX";
    prependToVar cmakeFlags "-DCMAKE_C_COMPILER=$CC";
    prependToVar cmakeFlags "-DCMAKE_AR=$(command -v $AR)";
    prependToVar cmakeFlags "-DCMAKE_RANLIB=$(command -v $RANLIB)";
    prependToVar cmakeFlags "-DCMAKE_STRIP=$(command -v $STRIP)";
    prependToVar cmakeFlags "-DCMAKE_INSTALL_NAME_DIR=${!outputLib}/lib";
    if [[ -z "$shareDocName" ]]; then
        local cmakeLists="${cmakeDir}/CMakeLists.txt";
        if [[ -f "$cmakeLists" ]]; then
            local shareDocName="$(grep --only-matching --perl-regexp --ignore-case '\bproject\s*\(\s*"?\K([^[:space:]")]+)' < "$cmakeLists" | head -n1)";
        fi;
        if [[ -z "$shareDocName" ]] || echo "$shareDocName" | grep -q '[^a-zA-Z0-9_+-]'; then
            if [[ -n "${pname-}" ]]; then
                shareDocName="$pname";
            else
                shareDocName="$(echo "$name" | sed 's/-[^a-zA-Z].*//')";
            fi;
        fi;
    fi;
    prependToVar cmakeFlags "-DCMAKE_INSTALL_BINDIR=${!outputBin}/bin";
    prependToVar cmakeFlags "-DCMAKE_INSTALL_SBINDIR=${!outputBin}/sbin";
    prependToVar cmakeFlags "-DCMAKE_INSTALL_INCLUDEDIR=${!outputInclude}/include";
    prependToVar cmakeFlags "-DCMAKE_INSTALL_MANDIR=${!outputMan}/share/man";
    prependToVar cmakeFlags "-DCMAKE_INSTALL_INFODIR=${!outputInfo}/share/info";
    prependToVar cmakeFlags "-DCMAKE_INSTALL_DOCDIR=${!outputDoc}/share/doc/${shareDocName}";
    prependToVar cmakeFlags "-DCMAKE_INSTALL_LIBDIR=${!outputLib}/lib";
    prependToVar cmakeFlags "-DCMAKE_INSTALL_LIBEXECDIR=${!outputLib}/libexec";
    prependToVar cmakeFlags "-DCMAKE_INSTALL_LOCALEDIR=${!outputLib}/share/locale";
    if [ -z "${doCheck-}" ]; then
        prependToVar cmakeFlags "-DBUILD_TESTING=OFF";
    fi;
    prependToVar cmakeFlags "-DCMAKE_BUILD_TYPE=${cmakeBuildType:-Release}";
    prependToVar cmakeFlags "-DCMAKE_EXPORT_NO_PACKAGE_REGISTRY=ON";
    prependToVar cmakeFlags "-DCMAKE_FIND_USE_PACKAGE_REGISTRY=OFF";
    prependToVar cmakeFlags "-DCMAKE_FIND_USE_SYSTEM_PACKAGE_REGISTRY=OFF";
    if [ "${buildPhase-}" = ninjaBuildPhase ]; then
        prependToVar cmakeFlags "-GNinja";
    fi;
    local flagsArray=();
    concatTo flagsArray cmakeFlags cmakeFlagsArray;
    echoCmd 'cmake flags' "${flagsArray[@]}";
    cmake "$cmakeDir" "${flagsArray[@]}";
    if ! [[ -v enableParallelBuilding ]]; then
        enableParallelBuilding=1;
        echo "cmake: enabled parallel building";
    fi;
    if [[ "$enableParallelBuilding" -ne 0 ]]; then
        export CMAKE_BUILD_PARALLEL_LEVEL=$NIX_BUILD_CORES;
    fi;
    if ! [[ -v enableParallelInstalling ]]; then
        enableParallelInstalling=1;
        echo "cmake: enabled parallel installing";
    fi;
    runHook postConfigure
}
isScript ()
{
 
    local fn="$1";
    local fd;
    local magic;
    exec {fd}< "$fn";
    LANG=C read -r -n 2 -u "$fd" magic;
    exec {fd}>&-;
    if [[ "$magic" =~ \#! ]]; then
        return 0;
    else
        return 1;
    fi
}
glibPreInstallPhase ()
{
 
    makeFlagsArray+=("gsettingsschemadir=${!outputLib}/share/gsettings-schemas/$name/glib-2.0/schemas/")
}
noBrokenSymlinksInAllOutputs ()
{
 
    if [[ -z ${dontCheckForBrokenSymlinks-} ]]; then
        for output in $(getAllOutputNames);
        do
            noBrokenSymlinks "${!output}";
        done;
    fi
}
_assignFirst ()
{
 
    local varName="$1";
    local _var;
    local REMOVE=REMOVE;
    shift;
    for _var in "$@";
    do
        if [ -n "${!_var-}" ]; then
            eval "${varName}"="${_var}";
            return;
        fi;
    done;
    echo;
    echo "error: _assignFirst: could not find a non-empty variable whose name to assign to ${varName}.";
    echo "       The following variables were all unset or empty:";
    echo "           $*";
    if [ -z "${out:-}" ]; then
        echo '       If you do not want an "out" output in your derivation, make sure to define';
        echo '       the other specific required outputs. This can be achieved by picking one';
        echo "       of the above as an output.";
        echo '       You do not have to remove "out" if you want to have a different default';
        echo '       output, because the first output is taken as a default.';
        echo;
    fi;
    return 1
}
findGdkPixbufLoaders ()
{
 
    local loadersCache="$1/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache";
    if [[ -f "$loadersCache" ]]; then
        if [[ -f "${GDK_PIXBUF_MODULE_FILE-}" ]]; then
            if (( "$(cat "$loadersCache" | wc -l)" > "$(cat "$GDK_PIXBUF_MODULE_FILE" | wc -l)" )); then
                export GDK_PIXBUF_MODULE_FILE="$loadersCache";
            fi;
        else
            export GDK_PIXBUF_MODULE_FILE="$loadersCache";
        fi;
    fi
}
runOneHook ()
{
 
    local hookName="$1";
    shift;
    local hooksSlice="${hookName%Hook}Hooks[@]";
    local hook ret=1;
    for hook in "_callImplicitHook 1 $hookName" ${!hooksSlice+"${!hooksSlice}"};
    do
        _logHook "$hookName" "$hook" "$@";
        if _eval "$hook" "$@"; then
            ret=0;
            break;
        fi;
    done;
    return "$ret"
}
make_glib_find_gsettings_schemas ()
{
 
    for maybe_dir in "$1"/share/gsettings-schemas/*;
    do
        if [[ -d "$maybe_dir/glib-2.0/schemas" ]]; then
            addToSearchPath GSETTINGS_SCHEMAS_PATH "$maybe_dir";
        fi;
    done
}
_doStrip ()
{
 
    local -ra flags=(dontStripHost dontStripTarget);
    local -ra debugDirs=(stripDebugList stripDebugListTarget);
    local -ra allDirs=(stripAllList stripAllListTarget);
    local -ra stripCmds=(STRIP STRIP_FOR_TARGET);
    local -ra ranlibCmds=(RANLIB RANLIB_FOR_TARGET);
    stripDebugList=${stripDebugList[*]:-lib lib32 lib64 libexec bin sbin Applications Library/Frameworks};
    stripDebugListTarget=${stripDebugListTarget[*]:-};
    stripAllList=${stripAllList[*]:-};
    stripAllListTarget=${stripAllListTarget[*]:-};
    local i;
    for i in ${!stripCmds[@]};
    do
        local -n flag="${flags[$i]}";
        local -n debugDirList="${debugDirs[$i]}";
        local -n allDirList="${allDirs[$i]}";
        local -n stripCmd="${stripCmds[$i]}";
        local -n ranlibCmd="${ranlibCmds[$i]}";
        if [[ -n "${dontStrip-}" || -n "${flag-}" ]] || ! type -f "${stripCmd-}" 2> /dev/null 1>&2; then
            continue;
        fi;
        stripDirs "$stripCmd" "$ranlibCmd" "$debugDirList" "${stripDebugFlags[*]:--S -p}";
        stripDirs "$stripCmd" "$ranlibCmd" "$allDirList" "${stripAllFlags[*]:--s -p}";
    done
}
_addRpathPrefix ()
{
 
    if [ "${NIX_NO_SELF_RPATH:-0}" != 1 ]; then
        export NIX_LDFLAGS="-rpath $1/lib ${NIX_LDFLAGS-}";
    fi
}
consumeEntire ()
{
 
    if IFS='' read -r -d '' "$1"; then
        echo "consumeEntire(): ERROR: Input null bytes, won't process" 1>&2;
        return 1;
    fi
}
runPhase ()
{
 
    local curPhase="$*";
    if [[ "$curPhase" = unpackPhase && -n "${dontUnpack:-}" ]]; then
        return;
    fi;
    if [[ "$curPhase" = patchPhase && -n "${dontPatch:-}" ]]; then
        return;
    fi;
    if [[ "$curPhase" = configurePhase && -n "${dontConfigure:-}" ]]; then
        return;
    fi;
    if [[ "$curPhase" = buildPhase && -n "${dontBuild:-}" ]]; then
        return;
    fi;
    if [[ "$curPhase" = checkPhase && -z "${doCheck:-}" ]]; then
        return;
    fi;
    if [[ "$curPhase" = installPhase && -n "${dontInstall:-}" ]]; then
        return;
    fi;
    if [[ "$curPhase" = fixupPhase && -n "${dontFixup:-}" ]]; then
        return;
    fi;
    if [[ "$curPhase" = installCheckPhase && -z "${doInstallCheck:-}" ]]; then
        return;
    fi;
    if [[ "$curPhase" = distPhase && -z "${doDist:-}" ]]; then
        return;
    fi;
    showPhaseHeader "$curPhase";
    dumpVars;
    local startTime endTime;
    startTime=$(date +"%s");
    eval "${!curPhase:-$curPhase}";
    endTime=$(date +"%s");
    showPhaseFooter "$curPhase" "$startTime" "$endTime";
    if [ "$curPhase" = unpackPhase ]; then
        [ -n "${sourceRoot:-}" ] && chmod +x -- "${sourceRoot}";
        cd -- "${sourceRoot:-.}";
    fi
}
justCheckPhase ()
{
 
    runHook preCheck;
    if [ -z "${checkTarget:-}" ]; then
        if just -n test > /dev/null 2>&1; then
            checkTarget="test";
        fi;
    fi;
    if [ -z "${checkTarget:-}" ]; then
        echo "no test target found in just, doing nothing";
    else
        local flagsArray=();
        concatTo flagsArray justFlags justFlagsArray checkTarget;
        echoCmd 'check flags' "${flagsArray[@]}";
        just "${flagsArray[@]}";
    fi;
    runHook postCheck
}
getHostRoleEnvHook ()
{
 
    getRole "$depHostOffset"
}
noBrokenSymlinks ()
{
 
    local -r output="${1:?}";
    local path;
    local pathParent;
    local symlinkTarget;
    local -i numDanglingSymlinks=0;
    local -i numReflexiveSymlinks=0;
    local -i numUnreadableSymlinks=0;
    if [[ ! -e $output ]]; then
        nixWarnLog "skipping non-existent output $output";
        return 0;
    fi;
    nixInfoLog "running on $output";
    while IFS= read -r -d '' path; do
        pathParent="$(dirname "$path")";
        if ! symlinkTarget="$(readlink "$path")"; then
            nixErrorLog "the symlink $path is unreadable";
            numUnreadableSymlinks+=1;
            continue;
        fi;
        if [[ $symlinkTarget == /* ]]; then
            nixInfoLog "symlink $path points to absolute target $symlinkTarget";
        else
            nixInfoLog "symlink $path points to relative target $symlinkTarget";
            symlinkTarget="$(realpath --no-symlinks --canonicalize-missing "$pathParent/$symlinkTarget")";
        fi;
        if [[ $symlinkTarget = "$TMPDIR"/* ]]; then
            nixErrorLog "the symlink $path points to $TMPDIR directory: $symlinkTarget";
            numDanglingSymlinks+=1;
            continue;
        fi;
        if [[ $symlinkTarget != "$NIX_STORE"/* ]]; then
            nixInfoLog "symlink $path points outside the Nix store; ignoring";
            continue;
        fi;
        if [[ $path == "$symlinkTarget" ]]; then
            nixErrorLog "the symlink $path is reflexive";
            numReflexiveSymlinks+=1;
        else
            if [[ ! -e $symlinkTarget ]]; then
                nixErrorLog "the symlink $path points to a missing target: $symlinkTarget";
                numDanglingSymlinks+=1;
            else
                nixDebugLog "the symlink $path is irreflexive and points to a target which exists";
            fi;
        fi;
    done < <(find "$output" -type l -print0);
    if ((numDanglingSymlinks > 0 || numReflexiveSymlinks > 0 || numUnreadableSymlinks > 0)); then
        nixErrorLog "found $numDanglingSymlinks dangling symlinks, $numReflexiveSymlinks reflexive symlinks and $numUnreadableSymlinks unreadable symlinks";
        exit 1;
    fi;
    return 0
}
_callImplicitHook ()
{
 
    local def="$1";
    local hookName="$2";
    if declare -F "$hookName" > /dev/null; then
        nixTalkativeLog "calling implicit '$hookName' function hook";
        "$hookName";
    else
        if type -p "$hookName" > /dev/null; then
            nixTalkativeLog "sourcing implicit '$hookName' script hook";
            source "$hookName";
        else
            if [ -n "${!hookName:-}" ]; then
                nixTalkativeLog "evaling implicit '$hookName' string hook";
                eval "${!hookName}";
            else
                return "$def";
            fi;
        fi;
    fi
}
_moveToShare ()
{
 
    if [ -n "$__structuredAttrs" ]; then
        if [ -z "${forceShare-}" ]; then
            forceShare=(man doc info);
        fi;
    else
        forceShare=(${forceShare:-man doc info});
    fi;
    if [[ -z "$out" ]]; then
        return;
    fi;
    for d in "${forceShare[@]}";
    do
        if [ -d "$out/$d" ]; then
            if [ -d "$out/share/$d" ]; then
                echo "both $d/ and share/$d/ exist!";
            else
                echo "moving $out/$d to $out/share/$d";
                mkdir -p $out/share;
                mv $out/$d $out/share/;
            fi;
        fi;
    done
}
getAllOutputNames ()
{
 
    if [ -n "$__structuredAttrs" ]; then
        echo "${!outputs[*]}";
    else
        echo "$outputs";
    fi
}
auditTmpdir ()
{
 
    local dir="$1";
    [ -e "$dir" ] || return 0;
    echo "checking for references to $TMPDIR/ in $dir...";
    local tmpdir elf_fifo script_fifo;
    tmpdir="$(mktemp -d)";
    elf_fifo="$tmpdir/elf";
    script_fifo="$tmpdir/script";
    mkfifo "$elf_fifo" "$script_fifo";
    ( find "$dir" -type f -not -path '*/.build-id/*' -print0 | while IFS= read -r -d '' file; do
        if isELF "$file"; then
            printf '%s\0' "$file" 1>&3;
        else
            if isScript "$file"; then
                filename=${file##*/};
                dir=${file%/*};
                if [ -e "$dir/.$filename-wrapped" ]; then
                    printf '%s\0' "$file" 1>&4;
                fi;
            fi;
        fi;
    done;
    exec 3>&- 4>&- ) 3> "$elf_fifo" 4> "$script_fifo" & ( xargs -0 -r -P "$NIX_BUILD_CORES" -n 1 sh -c '
            if { printf :; patchelf --print-rpath "$1"; } | grep -q -F ":$TMPDIR/"; then
                echo "RPATH of binary $1 contains a forbidden reference to $TMPDIR/"
                exit 1
            fi
        ' _ < "$elf_fifo" ) & local pid_elf=$!;
    local pid_script;
    ( xargs -0 -r -P "$NIX_BUILD_CORES" -n 1 sh -c '
            if grep -q -F "$TMPDIR/" "$1"; then
                echo "wrapper script $1 contains a forbidden reference to $TMPDIR/"
                exit 1
            fi
        ' _ < "$script_fifo" ) & local pid_script=$!;
    wait "$pid_elf" || { 
        echo "Some binaries contain forbidden references to $TMPDIR/. Check the error above!";
        exit 1
    };
    wait "$pid_script" || { 
        echo "Some scripts contain forbidden references to $TMPDIR/. Check the error above!";
        exit 1
    };
    rm -r "$tmpdir"
}
printLines ()
{
 
    (( "$#" > 0 )) || return 0;
    printf '%s\n' "$@"
}
_moveSbin ()
{
 
    if [ "${dontMoveSbin-}" = 1 ]; then
        return;
    fi;
    if [ ! -e "$prefix/sbin" -o -L "$prefix/sbin" ]; then
        return;
    fi;
    echo "moving $prefix/sbin/* to $prefix/bin";
    mkdir -p $prefix/bin;
    shopt -s dotglob;
    for i in $prefix/sbin/*;
    do
        mv "$i" $prefix/bin;
    done;
    shopt -u dotglob;
    rmdir $prefix/sbin;
    ln -s bin $prefix/sbin
}
updateAutotoolsGnuConfigScriptsPhase ()
{
 
    if [ -n "${dontUpdateAutotoolsGnuConfigScripts-}" ]; then
        return;
    fi;
    for script in config.sub config.guess;
    do
        for f in $(find . -type f -name "$script");
        do
            echo "Updating Autotools / GNU config script to a newer upstream version: $f";
            cp -f "/nix/store/dxagrpxl8wg2z16al3ls3zaci6dvhnnn-gnu-config-2024-01-01/$script" "$f";
        done;
    done
}
installPhase ()
{
 
    runHook preInstall;
    if [[ -z "${makeFlags-}" && -z "${makefile:-}" && ! ( -e Makefile || -e makefile || -e GNUmakefile ) ]]; then
        echo "no Makefile or custom installPhase, doing nothing";
        runHook postInstall;
        return;
    else
        foundMakefile=1;
    fi;
    if [ -n "$prefix" ]; then
        mkdir -p "$prefix";
    fi;
    local flagsArray=(${enableParallelInstalling:+-j${NIX_BUILD_CORES}} SHELL="$SHELL");
    concatTo flagsArray makeFlags makeFlagsArray installFlags installFlagsArray installTargets=install;
    echoCmd 'install flags' "${flagsArray[@]}";
    make ${makefile:+-f $makefile} "${flagsArray[@]}";
    unset flagsArray;
    runHook postInstall
}
unpackFile ()
{
 
    curSrc="$1";
    echo "unpacking source archive $curSrc";
    if ! runOneHook unpackCmd "$curSrc"; then
        echo "do not know how to unpack source archive $curSrc";
        exit 1;
    fi
}
getTargetRoleWrapper ()
{
 
    case $targetOffset in 
        -1)
            export NIX_BINTOOLS_WRAPPER_TARGET_BUILD_x86_64_unknown_linux_gnu=1
        ;;
        0)
            export NIX_BINTOOLS_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu=1
        ;;
        1)
            export NIX_BINTOOLS_WRAPPER_TARGET_TARGET_x86_64_unknown_linux_gnu=1
        ;;
        *)
            echo "mold-unwrapped-wrapper-2.42.0: used as improper sort of dependency" 1>&2;
            return 1
        ;;
    esac
}
nixErrorLog ()
{
 
    _nixLogWithLevel 0 "$*"
}
dropIconThemeCache ()
{
 
    if [[ -z "${dontDropIconThemeCache:-}" ]]; then
        local icondir="${out:?}/share/icons";
        if [[ -d "${icondir}" ]]; then
            find "${icondir}" -name 'icon-theme.cache' -print0 | while IFS= read -r -d '' file; do
                echo "Removing ${file}";
                rm -f "${file}";
            done;
        fi;
    fi
}
addToSearchPath ()
{
 
    addToSearchPathWithCustomDelimiter ":" "$@"
}
_multioutConfig ()
{
 
    if [ "$(getAllOutputNames)" = "out" ] || [ -z "${setOutputFlags-1}" ]; then
        return;
    fi;
    if [ -z "${shareDocName:-}" ]; then
        local confScript="${configureScript:-}";
        if [ -z "$confScript" ] && [ -x ./configure ]; then
            confScript=./configure;
        fi;
        if [ -f "$confScript" ]; then
            local shareDocName="$(sed -n "s/^PACKAGE_TARNAME='\(.*\)'$/\1/p" < "$confScript")";
        fi;
        if [ -z "$shareDocName" ] || echo "$shareDocName" | grep -q '[^a-zA-Z0-9_-]'; then
            shareDocName="$(echo "$name" | sed 's/-[^a-zA-Z].*//')";
        fi;
    fi;
    prependToVar configureFlags --bindir="${!outputBin}"/bin --sbindir="${!outputBin}"/sbin --includedir="${!outputInclude}"/include --mandir="${!outputMan}"/share/man --infodir="${!outputInfo}"/share/info --docdir="${!outputDoc}"/share/doc/"${shareDocName}" --libdir="${!outputLib}"/lib --libexecdir="${!outputLib}"/libexec --localedir="${!outputLib}"/share/locale;
    prependToVar installFlags pkgconfigdir="${!outputDev}"/lib/pkgconfig m4datadir="${!outputDev}"/share/aclocal aclocaldir="${!outputDev}"/share/aclocal
}
nixVomitLog ()
{
 
    _nixLogWithLevel 7 "$*"
}
populateBindgenEnv ()
{
 
    export LIBCLANG_PATH=/nix/store/yc2a9854a2y2c8kci88piblc847iq1l4-clang-21.1.8-lib/lib;
    BINDGEN_EXTRA_CLANG_ARGS="$(< /nix/store/w021fbcg4z6vxihnp6gb6vijyifl051f-clang-wrapper-21.1.8/nix-support/cc-cflags) $(< /nix/store/w021fbcg4z6vxihnp6gb6vijyifl051f-clang-wrapper-21.1.8/nix-support/libc-cflags) $(< /nix/store/w021fbcg4z6vxihnp6gb6vijyifl051f-clang-wrapper-21.1.8/nix-support/libcxx-cxxflags) $NIX_CFLAGS_COMPILE";
    export BINDGEN_EXTRA_CLANG_ARGS
}
_moveSystemdUserUnits ()
{
 
    if [ "${dontMoveSystemdUserUnits:-0}" = 1 ]; then
        return;
    fi;
    if [ ! -e "${prefix:?}/lib/systemd/user" ]; then
        return;
    fi;
    local source="$prefix/lib/systemd/user";
    local target="$prefix/share/systemd/user";
    echo "moving $source/* to $target";
    mkdir -p "$target";
    ( shopt -s dotglob;
    for i in "$source"/*;
    do
        mv "$i" "$target";
    done );
    rmdir "$source";
    ln -s "$target" "$source"
}
getTargetRole ()
{
 
    getRole "$targetOffset"
}
patchShebangsAuto ()
{
 
    if [[ -z "${dontPatchShebangs-}" && -e "$prefix" ]]; then
        if [[ "$output" != out && "$output" = "$outputDev" ]]; then
            patchShebangs --build "$prefix";
        else
            patchShebangs --host "$prefix";
        fi;
    fi
}
_nixLogWithLevel ()
{
 
    [[ -z ${NIX_LOG_FD-} || ${NIX_DEBUG:-0} -lt ${1:?} ]] && return 0;
    local logLevel;
    case "${1:?}" in 
        0)
            logLevel=ERROR
        ;;
        1)
            logLevel=WARN
        ;;
        2)
            logLevel=NOTICE
        ;;
        3)
            logLevel=INFO
        ;;
        4)
            logLevel=TALKATIVE
        ;;
        5)
            logLevel=CHATTY
        ;;
        6)
            logLevel=DEBUG
        ;;
        7)
            logLevel=VOMIT
        ;;
        *)
            echo "_nixLogWithLevel: called with invalid log level: ${1:?}" >&"$NIX_LOG_FD";
            return 1
        ;;
    esac;
    local callerName="${FUNCNAME[2]}";
    if [[ $callerName == "_callImplicitHook" ]]; then
        callerName="${hookName:?}";
    fi;
    printf "%s: %s: %s\n" "$logLevel" "$callerName" "${2:?}" >&"$NIX_LOG_FD"
}
fixupPhase ()
{
 
    local output;
    for output in $(getAllOutputNames);
    do
        if [ -e "${!output}" ]; then
            chmod -R u+w,u-s,g-s "${!output}";
        fi;
    done;
    runHook preFixup;
    local output;
    for output in $(getAllOutputNames);
    do
        prefix="${!output}" runHook fixupOutput;
    done;
    recordPropagatedDependencies;
    if [ -n "${setupHook:-}" ]; then
        mkdir -p "${!outputDev}/nix-support";
        substituteAll "$setupHook" "${!outputDev}/nix-support/setup-hook";
    fi;
    if [ -n "${setupHooks:-}" ]; then
        mkdir -p "${!outputDev}/nix-support";
        local hook;
        for hook in ${setupHooks[@]};
        do
            local content;
            consumeEntire content < "$hook";
            substituteAllStream content "file '$hook'" >> "${!outputDev}/nix-support/setup-hook";
            unset -v content;
        done;
        unset -v hook;
    fi;
    if [ -n "${propagatedUserEnvPkgs[*]:-}" ]; then
        mkdir -p "${!outputBin}/nix-support";
        printWords "${propagatedUserEnvPkgs[@]}" > "${!outputBin}/nix-support/propagated-user-env-packages";
    fi;
    runHook postFixup
}
nixLog ()
{
 
    [[ -z ${NIX_LOG_FD-} ]] && return 0;
    local callerName="${FUNCNAME[1]}";
    if [[ $callerName == "_callImplicitHook" ]]; then
        callerName="${hookName:?}";
    fi;
    printf "%s: %s\n" "$callerName" "$*" >&"$NIX_LOG_FD"
}
_addToEnv ()
{
 
    local depHostOffset depTargetOffset;
    local pkg;
    for depHostOffset in "${allPlatOffsets[@]}";
    do
        local hookVar="${pkgHookVarVars[depHostOffset + 1]}";
        local pkgsVar="${pkgAccumVarVars[depHostOffset + 1]}";
        for depTargetOffset in "${allPlatOffsets[@]}";
        do
            (( depHostOffset <= depTargetOffset )) || continue;
            local hookRef="${hookVar}[$depTargetOffset - $depHostOffset]";
            if [[ -z "${strictDeps-}" ]]; then
                local visitedPkgs="";
                for pkg in "${pkgsBuildBuild[@]}" "${pkgsBuildHost[@]}" "${pkgsBuildTarget[@]}" "${pkgsHostHost[@]}" "${pkgsHostTarget[@]}" "${pkgsTargetTarget[@]}";
                do
                    if [[ "$visitedPkgs" = *"$pkg"* ]]; then
                        continue;
                    fi;
                    runHook "${!hookRef}" "$pkg";
                    visitedPkgs+=" $pkg";
                done;
            else
                local pkgsRef="${pkgsVar}[$depTargetOffset - $depHostOffset]";
                local pkgsSlice="${!pkgsRef}[@]";
                for pkg in ${!pkgsSlice+"${!pkgsSlice}"};
                do
                    runHook "${!hookRef}" "$pkg";
                done;
            fi;
        done;
    done
}
_pruneLibtoolFiles ()
{
 
    if [ "${dontPruneLibtoolFiles-}" ] || [ ! -e "$prefix" ]; then
        return;
    fi;
    find "$prefix" -type f -name '*.la' -exec grep -q '^# Generated by .*libtool' {} \; -exec grep -q "^old_library=''" {} \; -exec sed -i {} -e "/^dependency_libs='[^']/ c dependency_libs='' #pruned" \;
}
justBuildPhase ()
{
 
    runHook preBuild;
    local flagsArray=();
    concatTo flagsArray justFlags justFlagsArray;
    echoCmd 'build flags' "${flagsArray[@]}";
    just "${flagsArray[@]}";
    runHook postBuild
}
_activatePkgs ()
{
 
    local hostOffset targetOffset;
    local pkg;
    for hostOffset in "${allPlatOffsets[@]}";
    do
        local pkgsVar="${pkgAccumVarVars[hostOffset + 1]}";
        for targetOffset in "${allPlatOffsets[@]}";
        do
            (( hostOffset <= targetOffset )) || continue;
            local pkgsRef="${pkgsVar}[$targetOffset - $hostOffset]";
            local pkgsSlice="${!pkgsRef}[@]";
            for pkg in ${!pkgsSlice+"${!pkgsSlice}"};
            do
                activatePackage "$pkg" "$hostOffset" "$targetOffset";
            done;
        done;
    done
}
glibPostInstallHook ()
{
 
    if [ -d "$prefix/share/glib-2.0/schemas" ]; then
        mkdir -p "${!outputLib}/share/gsettings-schemas/$name/glib-2.0";
        mv "$prefix/share/glib-2.0/schemas" "${!outputLib}/share/gsettings-schemas/$name/glib-2.0/";
    fi;
    addToSearchPath GSETTINGS_SCHEMAS_PATH "${!outputLib}/share/gsettings-schemas/$name"
}
_gtkCleanImmodulesCache ()
{
 
    local f="${prefix:?}/lib/gtk-3.0/3.0.0/immodules.cache";
    if [ -f "$f" ]; then
        sed 's|Created by .*bin/gtk-query-|Created by bin/gtk-query-|' -i "$f";
    fi
}
prependToVar ()
{
 
    local -n nameref="$1";
    local useArray type;
    if [ -n "$__structuredAttrs" ]; then
        useArray=true;
    else
        useArray=false;
    fi;
    if type=$(declare -p "$1" 2> /dev/null); then
        case "${type#* }" in 
            -A*)
                echo "prependToVar(): ERROR: trying to use prependToVar on an associative array." 1>&2;
                return 1
            ;;
            -a*)
                useArray=true
            ;;
            *)
                useArray=false
            ;;
        esac;
    fi;
    shift;
    if $useArray; then
        nameref=("$@" ${nameref+"${nameref[@]}"});
    else
        nameref="$* ${nameref-}";
    fi
}
gettextDataDirsHook ()
{
 
    getHostRoleEnvHook;
    if [ -d "$1/share/gettext" ]; then
        addToSearchPath "GETTEXTDATADIRS${role_post}" "$1/share/gettext";
    fi
}
concatTo ()
{
 
    local -;
    set -o noglob;
    local -n targetref="$1";
    shift;
    local arg default name type;
    for arg in "$@";
    do
        IFS="=" read -r name default <<< "$arg";
        local -n nameref="$name";
        if [[ -z "${nameref[*]}" && -n "$default" ]]; then
            targetref+=("$default");
        else
            if type=$(declare -p "$name" 2> /dev/null); then
                case "${type#* }" in 
                    -A*)
                        echo "concatTo(): ERROR: trying to use concatTo on an associative array." 1>&2;
                        return 1
                    ;;
                    -a*)
                        targetref+=("${nameref[@]}")
                    ;;
                    *)
                        if [[ "$name" = *"Array" ]]; then
                            nixErrorLog "concatTo(): $name is not declared as array, treating as a singleton. This will become an error in future";
                            targetref+=(${nameref+"${nameref[@]}"});
                        else
                            targetref+=(${nameref-});
                        fi
                    ;;
                esac;
            fi;
        fi;
    done
}
nixDebugLog ()
{
 
    _nixLogWithLevel 6 "$*"
}
PATH="$PATH${nix_saved_PATH:+:$nix_saved_PATH}"
XDG_DATA_DIRS="$XDG_DATA_DIRS${nix_saved_XDG_DATA_DIRS:+:$nix_saved_XDG_DATA_DIRS}"

eval "${shellHook:-}"
shopt -s expand_aliases

eval "set --; exec $1"
