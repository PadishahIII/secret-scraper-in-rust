let SessionLoad = 1
let s:so_save = &g:so | let s:siso_save = &g:siso | setg so=0 siso=0 | setl so=-1 siso=-1
let v:this_session=expand("<sfile>:p")
silent only
silent tabonly
cd /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust
if expand('%') == '' && !&modified && line('$') <= 1 && getline(1) == ''
  let s:wipebuf = bufnr('%')
endif
let s:shortmess_save = &shortmess
if &shortmess =~ 'A'
  set shortmess=aoOA
else
  set shortmess=aoO
endif
badd +167 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/src/urlparser.rs
badd +11 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/src/handler.rs
badd +1201 ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/url-2.5.8/src/lib.rs
badd +19 src/cli.rs
badd +4 src/main.rs
badd +4 src/lib.rs
badd +46 ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/url-2.5.8/src/host.rs
badd +5 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/src/output.rs
badd +56 ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/trim-in-place-0.1.7/src/lib.rs
badd +164 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/tests/urlparser_test.rs
badd +863 ~/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/src/rust/library/std/src/panicking.rs
argglobal
%argdel
$argadd .
edit /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/tests/urlparser_test.rs
let s:save_splitbelow = &splitbelow
let s:save_splitright = &splitright
set splitbelow splitright
wincmd _ | wincmd |
split
1wincmd k
wincmd w
let &splitbelow = s:save_splitbelow
let &splitright = s:save_splitright
wincmd t
let s:save_winminheight = &winminheight
let s:save_winminwidth = &winminwidth
set winminheight=0
set winheight=1
set winminwidth=0
set winwidth=1
exe '1resize ' . ((&lines * 47 + 31) / 63)
exe '2resize ' . ((&lines * 12 + 31) / 63)
argglobal
balt /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/src/urlparser.rs
setlocal foldmethod=manual
setlocal foldexpr=0
setlocal foldmarker={{{,}}}
setlocal foldignore=#
setlocal foldlevel=0
setlocal foldminlines=1
setlocal foldnestmax=20
setlocal foldenable
silent! normal! zE
let &fdl = &fdl
let s:l = 161 - ((35 * winheight(0) + 23) / 47)
if s:l < 1 | let s:l = 1 | endif
keepjumps exe s:l
normal! zt
keepjumps 161
normal! 07|
wincmd w
argglobal
if bufexists(fnamemodify("term:///Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust//53891:zsh;\#toggleterm\#1", ":p")) | buffer term:///Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust//53891:zsh;\#toggleterm\#1 | else | edit term:///Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust//53891:zsh;\#toggleterm\#1 | endif
if &buftype ==# 'terminal'
  silent file term:///Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust//53891:zsh;\#toggleterm\#1
endif
balt /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/tests/urlparser_test.rs
setlocal foldmethod=manual
setlocal foldexpr=0
setlocal foldmarker={{{,}}}
setlocal foldignore=#
setlocal foldlevel=0
setlocal foldminlines=1
setlocal foldnestmax=20
setlocal foldenable
let s:l = 2056 - ((11 * winheight(0) + 6) / 12)
if s:l < 1 | let s:l = 1 | endif
keepjumps exe s:l
normal! zt
keepjumps 2056
normal! 0
wincmd w
exe '1resize ' . ((&lines * 47 + 31) / 63)
exe '2resize ' . ((&lines * 12 + 31) / 63)
tabnext 1
if exists('s:wipebuf') && len(win_findbuf(s:wipebuf)) == 0 && getbufvar(s:wipebuf, '&buftype') isnot# 'terminal'
  silent exe 'bwipe ' . s:wipebuf
endif
unlet! s:wipebuf
set winheight=1 winwidth=20
let &shortmess = s:shortmess_save
let &winminheight = s:save_winminheight
let &winminwidth = s:save_winminwidth
let s:sx = expand("<sfile>:p:r")."x.vim"
if filereadable(s:sx)
  exe "source " . fnameescape(s:sx)
endif
let &g:so = s:so_save | let &g:siso = s:siso_save
set hlsearch
doautoall SessionLoadPost
unlet SessionLoad
" vim: set ft=vim :
