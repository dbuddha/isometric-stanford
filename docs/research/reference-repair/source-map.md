# Source map

| ID | Kind and identity | Exact use | Limits |
| --- | --- | --- | --- |
| S-001 | [Isometric NYC project account](https://cannoneyed.com/projects/isometric-nyc), reviewed 2026-08-30 | Closest workflow precedent for Google orthographic capture, Qwen translation, neighboring-art infill, dashboard review, DZI, cost, and defects | Practitioner account, not a deterministic benchmark |
| S-002 | [Isometric NYC source at `0084463`](https://github.com/cannoneyed/isometric-nyc/tree/008446357ec67512c4329d25edefb6c508c7b24d) | Source-pinned capture, generation, infill-template, queue, and review behavior | MIT code; generated imagery has separate rights |
| S-003 | [Guided Image Filtering, ECCV 2010](https://mlanthology.org/eccv/2010/he2010eccv-guided/) | Edge-aware smoothing precedent with a linear-time local model | Does not identify materials or repair geometry |
| S-004 | [Image Smoothing via L0 Gradient Minimization, SIGGRAPH Asia 2011](https://researchportal.hkust.edu.hk/en/publications/image-smoothing-via-lsub0sub-gradient-minimization/) | Structure-sparsifying baseline for removing small gradients | Global optimization is more complex to make exact and bounded |
| S-005 | [Rolling Guidance Filter, ECCV 2014](https://mlanthology.org/eccv/2014/zhang2014eccv-rolling/) | Iterative scale-aware removal of small structures while recovering major edges | Iteration and parameters do not add semantic understanding |
| S-006 | [SNIC, CVPR 2017](https://openaccess.thecvf.com/content_cvpr_2017/html/Achanta_Superpixels_and_Polygons_CVPR_2017_paper.html) | Linear-complexity superpixels and polygonal region candidates for later material boundaries | Superpixels follow appearance and can cross semantic boundaries |
| S-007 | [Mask2Former, CVPR 2022](https://openaccess.thecvf.com/content/CVPR2022/html/Cheng_Masked-Attention_Mask_Transformer_for_Universal_Image_Segmentation_CVPR_2022_paper.html) | Learned mask-classification architecture for a later Stanford surface benchmark | Requires suitable labels, weights, and domain evaluation |
| S-008 | [Segment Anything, ICCV 2023](https://openaccess.thecvf.com/content/ICCV2023/html/Kirillov_Segment_Anything_ICCV_2023_paper.html) | Prompt-refined instance boundaries and interactive mask correction | It proposes masks, not trusted semantic class identity |
| S-009 | [Segment Anything from Space, WACV 2024](https://openaccess.thecvf.com/content/WACV2024/html/Ren_Segment_Anything_From_Space_WACV_2024_paper.html) | Evidence that general segmentation models need overhead-domain evaluation | Stanford views are oblique photogrammetry, not the paper's complete domain |
| E-001 | Frozen private Hoover bundle `sample-sse8-125mm` | Exact registered color, depth, and normal source for all three candidates | Private Google-derived evidence, one place and one source epoch |
| E-002 | Repair report SHA-256 `579b664690c36994a529c183689271fcc3131a767a7b255662f5794a918b1ff0` | Three-run exact candidate and metric evidence | Does not include accepted semantic labels or an art approval |

## Independence and triangulation

S-003 through S-006 cover deterministic classical image processing. S-007
through S-009 cover learned and interactive mask proposals. S-001 and S-002
define the nearest product comparator. E-001 and E-002 measure this project's
actual Stanford input and implementation. The conclusion does not depend on
one paper or one qualitative screenshot.
