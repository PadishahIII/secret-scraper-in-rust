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
badd +347 src/output.rs
badd +1 ./
badd +491 src/scraper/crawler.rs
badd +163 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/tests/output_tests.rs
badd +651 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/tests/config_tests.rs
badd +174 src/scraper/worker.rs
badd +41 src/rate_limiter.rs
badd +32 Cargo.toml
badd +344 tests/crawler_facade_tests.rs
badd +148 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/tests/file_scanner_facade_tests.rs
badd +26 setting.yaml
badd +263 src/urlparser.rs
badd +140 tests/urlparser_test.rs
badd +55 src/cli.rs
badd +21 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/src/handler.rs
badd +1582 ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/url-2.5.8/src/lib.rs
badd +194 ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/urlparse-0.7.3/src/url.rs
badd +294 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/tests/binary_tests.rs
badd +534 /Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust/tests/cli_tests.rs
argglobal
%argdel
$argadd ./
edit src/urlparser.rs
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
wincmd =
argglobal
balt ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/url-2.5.8/src/lib.rs
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
let s:l = 264 - ((22 * winheight(0) + 16) / 33)
if s:l < 1 | let s:l = 1 | endif
keepjumps exe s:l
normal! zt
keepjumps 264
normal! 049|
wincmd w
argglobal
if bufexists(fnamemodify("term:///Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust//16402:zsh;\#toggleterm\#1", ":p")) | buffer term:///Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust//16402:zsh;\#toggleterm\#1 | else | edit term:///Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust//16402:zsh;\#toggleterm\#1 | endif
if &buftype ==# 'terminal'
  silent file term:///Volumes/T9/machines/Users/jasonharris/Documents/workspace/secret-scraper-in-rust//16402:zsh;\#toggleterm\#1
endif
balt src/urlparser.rs
setlocal foldmethod=manual
setlocal foldexpr=0
setlocal foldmarker={{{,}}}
setlocal foldignore=#
setlocal foldlevel=0
setlocal foldminlines=1
setlocal foldnestmax=20
setlocal foldenable
let s:l = 10022 - ((21 * winheight(0) + 15) / 31)
if s:l < 1 | let s:l = 1 | endif
keepjumps exe s:l
normal! zt
keepjumps 10022
normal! 023|
wincmd w
wincmd =
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
nohlsearch
doautoall SessionLoadPost
unlet SessionLoad
" vim: set ft=vim :
