# Research

Done 2026-11-11 with web searches. Confidence: H (primary source read or several agreeing sources), M (secondary source, or summary only), L (single weak source). Many rule findings come from law-firm and trade summaries, not the rule texts; those are marked M and are checked again before they go in the README. Vendor claims are labelled as vendor claims.

## 1. The problem: missed follow-up care, and what reminders do

### How often follow-up is missed

| Finding | Source | Confidence |
|---|---|---|
| In 23 US primary care practices, 135 of 1,889 abnormal test results (7.1%, about 1 in 14) had no record that the patient was told | Casalino et al., Arch Intern Med 2009, via [Medical Economics](https://medicaleconomics.com/view/study-doctors-often-fail-inform-patients-abnormal-test-results) and [NEJM Journal Watch](https://clinician.nejm.org/JW200907230000002) (which reports 6% by a different count) | M |
| 7–8% of abnormal-result alerts in an EHR had no sign of timely follow-up, even when the clinician had acknowledged the alert | Singh et al., summarised in [PMC3721157](https://pmc.ncbi.nlm.nih.gov/articles/PMC3721157) | M |
| 29.8% of 2,590 VA primary care practitioners said they had personally missed results that delayed care (self-report) | Survey via [Fierce Healthcare](https://www.fiercehealthcare.com/ehr/alert-overload-causing-docs-to-miss-abnormal-test-results) | M |
| After an abnormal stool test (FIT), the follow-up colonoscopy within 6 months is often not done: 18–57% completion across 8 US community health centre systems; 50.5% in a mailed-FIT program; 21% rising to 38% with patient navigation at one centre | [PMC7008958](https://pmc.ncbi.nlm.nih.gov/articles/PMC7008958), [J Prim Care Community Health 2024](https://doaj.org/article/8f83edf2606a41328293f76eb505faee), [UCSF 2016](https://www.ucsf.edu/news/2016/12/405236/many-safety-net-patients-dont-complete-colonoscopy-after-abnormal-stool-based) | M |
| Of Medicare patients readmitted within 30 days of a medical discharge, 50.2% had no physician bill between discharge and readmission (2003–04 claims) | Jencks et al., NEJM 2009, via [Commonwealth Fund](https://www.commonwealthfund.org/press-release/2009/new-study-20-percent-hospitalized-medicare-patients-readmitted-hospital-within) | M |
| A primary care visit within 7 days of discharge was linked to 12–24% lower odds of 30-day readmission for medicine patients, not for surgical ones (observational) | Kaiser study, JAMA Intern Med 2017, via [Fierce Healthcare](https://www.fiercehealthcare.com/practices/a-focused-primary-care-visit-after-hospital-stay-lowers-risk-readmission) | M |
| 61.4% of US adults aged 45–75 were up to date with colorectal screening in 2022 (target 72.8%); 29.8% at ages 45–49 | [CDC Preventing Chronic Disease, 2025](https://www.cdc.gov/pcd/issues/2025/25_0175.htm) | H |

### What reminders do

| Finding | Source | Confidence |
|---|---|---|
| Text reminders raise appointment attendance versus none: RR 1.14 (95% CI 1.03–1.26), 7 studies, 5,841 people; 67.8% → 78.6%. About the same as phone calls (RR 0.99) and cheaper. Evidence moderate; harms and message content poorly reported | Gurol-Urganci et al., [Cochrane Review CD007458, 2013](https://www.cochranelibrary.com/cdsr/doi/10.1002/14651858.CD007458/abstract) | H |
| Client reminders raise cancer screening: median +14.0 points for mammography (19 studies), +10.2 for Pap tests, +11.5 for stool tests; phone reminders beat written ones. Reviews date from 2010–2013; text and email are an open gap | [Community Preventive Services Task Force](https://www.thecommunityguide.org/findings/cancer-screening-client-reminders-breast-cancer), [cervical](https://thecommunityguide.org/findings/cancer-screening-client-reminders-cervical-cancer), [CDC planning guide](https://www.cdc.gov/screenoutcancer/ebi-planning-guides/client-reminder-planning-guide.htm) | H |
| A megastudy of 19 text messages before primary care visits (47,306 patients) raised flu vaccination by about 5% on average; the best said the shot was "reserved for you" and was sent twice; messages that matched what patients expect from their clinic did better | Milkman et al., [PNAS 2021](https://www.hks.harvard.edu/publications/megastudy-text-based-nudges-encouraging-patients-get-vaccinated-upcoming-doctors) | H |
| Telling high-risk patients their risk raised flu vaccination slightly; saying an algorithm found the risk added nothing | Geisinger trials, via [ThePrint](https://theprint.in/feature/could-a-personalised-flu-warning-get-more-people-vaccinated-us-study-says-yes/3063311/) and protocol [NCT05009251](https://cdn.clinicaltrials.gov/large-docs/51/NCT05009251/Prot_001.pdf); effect size **not found** in a published paper | L |
| A randomised trial of reminders whose text was written by a language model: **not found**. The closest is a simulation study of LLM-driven outreach (authors tied to an outreach company) | Basu et al., npj Digital Medicine 2024, via [search summary](https://x-mol.com/paper/1858756897987194880) | L |

**What this means for the design (recommendation):** follow-up is missed often enough to matter (roughly 1 in 14 abnormal results not communicated; half or fewer of positive stool tests followed by colonoscopy in safety-net settings), and plain reminders have solid, moderate-sized effects. There is no trial evidence that AI-written wording beats a good plain template. So the value of the model is saving staff time on wording, not a proven lift; the design should be able to compare AI drafts with a fixed template, and must never claim better attendance than the evidence shows.

## 2. Rules that touch reminders (US)

| Rule | What it means here | Source | Confidence |
|---|---|---|---|
| HIPAA: reminders are treatment | Appointment reminders are part of treatment and need no patient authorization; reminders for appointments, annual exams or refills are not marketing | [HHS FAQ 198](https://www.hhs.gov/hipaa/for-professionals/faq/198/may-health-care-providers-leave-messages/index.html) | H |
| HIPAA: say little | For messages others may hear or see, HHS suggests leaving only the provider's name, number and what is needed to confirm the appointment, or asking for a call back | [HHS FAQ 198](https://www.hhs.gov/hipaa/for-professionals/faq/198/may-health-care-providers-leave-messages/index.html) | H |
| HIPAA: minimum necessary | Does not apply to disclosures to the patient or to providers for treatment (45 CFR 164.502(b)(2)); the real risk is incidental disclosure to someone else reading the message | [HHS minimum necessary guidance](https://www.hhs.gov/sites/default/files/ocr/privacy/hipaa/understanding/coveredentities/minimumnecessary.pdf) | H |
| HIPAA: email | Unencrypted email to patients is not forbidden; providers should limit the content and check the address. A patient who objects must be offered another channel | [HHS email FAQ](https://www.hhs.gov/hipaa/for-professionals/faq/does-hipaa-permit-health-care-providers-to-use-email-to-discuss-health-issues-with-patients/index.html) | H |
| HIPAA: confidential communications (164.522(b)) | Providers must honour reasonable requests for how and where to be contacted (e.g. email not postcard, a different address) | [HHS FAQ 198](https://www.hhs.gov/hipaa/for-professionals/faq/198/may-health-care-providers-leave-messages/index.html) | H |
| TCPA healthcare exemption (FCC order of 2015-07-10, FCC 15-72) | Automated calls and texts to mobiles without prior consent are allowed for appointment and exam reminders, wellness checkups, lab results and post-discharge follow-up, if: free to the patient; only to the number the patient gave; names the provider and a contact number; no marketing, billing or debt content; text ≤ 160 characters; at most 1 message a day and 3 a week per provider; STOP opt-out honoured at once. The eCFR citation was not verified | [Mintz, 2015-07-27](https://www.mintz.com/insights-center/viewpoints/2146/2015-07-fcc-ruling-addresses-robocalls-health-care-providers) | M |
| FCC consent-revocation rules (2024 order) | Consumers may revoke consent "by any reasonable means"; revocations must be honoured within 10 business days. Most of it took effect 2025-04-10/11; "revoke all" was delayed to 2026-04-11, then 2027-01-31 | [Troutman, 2026-09-17](https://www.troutman.com/insights/fcc-revises-tcpa-revocation-of-consent-rules-that-were-set-to-go-into-effect-in-january/), [Kelley Drye](https://www.kelleydrye.com/viewpoints/blogs/ad-law-access/fcc-issues-one-year-limited-waiver-of-new-tcpa-consent-revocation-rule) | M |
| FCC 26-67 (adopted 2026-09-30, in Federal Register 2026-10-26, effective **2026-11-25**) | Replaces "revoke all": a STOP to an informational message may apply only to that category; callers may name an exclusive opt-out method. A further proposal (7 business days) is open for comment | [Brownstein](https://www.bhfs.com/insight/new-tcpa-revocation-of-consent-rules-take-effect-on-november-25/), [Inside Global Tech, 2026-10-02](https://www.insideglobaltech.com/2026/10/02/update-fcc-adopts-final-order-and-further-notice-on-tcpa-consent-revocation/) | M |
| CAN-SPAM | Covers email whose primary purpose is commercial; transactional or relationship messages are exempt from most of it (but not from false routing information). A reminder that adds promotions at the top becomes commercial | [FTC compliance guide](https://www.ftc.gov/business-guidance/resources/can-spam-act-compliance-guide-business) | H |
| A2P 10DLC (carrier rule, not law) | Every business text from a US 10-digit number must have a registered brand and campaign; unregistered traffic has been fully blocked since 2023-09-01. Campaigns that mix reminders and promotions fail vetting (vendor claim) | [Twilio changelog](https://www.twilio.com/en-us/changelog/-u-s--a2p-10dlc--full-blocking-of-traffic-sent-from-unregistered), [Twilio support](https://support.twilio.com/hc/en-us/articles/14910496447771-Shutdown-of-Unregistered-10DLC-Messaging) | H |
| California AB 3030 (Health & Safety Code 1339.75, in force 2025-01-01) | AI-generated messages about "patient clinical information" need a disclaimer at the start of written messages and a way to reach a human, unless a licensed or certified provider reads and reviews them. Appointment scheduling and other administrative matters are excluded from "clinical information"; a reminder that names a test result or reason for care may not be | Statute text via [california.public.law](https://california.public.law/codes/ca_health_and_safety_code_section_1339.75); official leginfo page blocked to us | M-H |
| Other states (Texas TRAIGA / SB 1188, 2026-01-01) | AI disclosure in diagnosis or treatment; scope for reminders unclear | [Akerman](https://www.akerman.com/en/perspectives/hrx-new-year-new-ai-rules-healthcare-ai-laws-now-in-effect.html) (from the lab project) | M |
| FDA: CDS guidance (2026-01-06) and the administrative-support exclusion (FD&C Act 520(o)(1)(A)) | Scheduling and administrative software is outside device rules. Software that decides who clinically needs care, prioritises patients or gives a single recommendation moves toward a device. A tool that applies the clinic's own stated rules ("recall 12 months after X") and shows the reason to staff stays on the safer side; whether this is exactly how FDA sees it was **not verified** | [Covington, 2026-01](https://www.cov.com/en/news-and-insights/insights/2026/01/5-key-takeaways-from-fdas-revised-clinical-decision-support-cds-software-guidance), [Bass Berry](https://bassberry.com/news/drawing-the-device-line-fda-seeks-public-input-on-patient-safety-and-non-device-health-software/) | M |

The demo uses a made-up clinic and patients, so none of these applies to it directly; they shape what the design must show to be credible.

**What this means for the design (recommendation):** email reminders under HIPAA are allowed if they say little; the engine already sends email. SMS adds three things the engine does not have: the TCPA exemption's limits (160 characters, 1 a day and 3 a week per clinic, provider name and number, STOP honoured at once), per-category opt-out under the new FCC rule, and 10DLC registration, which a demo cannot do for real. A person approving every message keeps AB 3030's disclaimer from applying, but the message should still say how it was made. "Who is due" should come from rules the clinic writes, shown with their reason, not from the model.

## 3. Data for a demo

| Finding | Source | Confidence |
|---|---|---|
| FHIR R4 has the pieces: `ServiceRequest` (a follow-up or screening ordered), `CarePlan` (planned activities), `Appointment` (booked visit), `Task` (work to do), `CommunicationRequest` (a request to send a message) and `Communication` (a record that it was sent); `Observation` and `Encounter` (e.g. a discharge) are the triggers | [CommunicationRequest R4](https://hl7.org/fhir/R4/communicationrequest.html), [Communication](https://hl7.org/fhir/communication.html); others from the base spec, read before | H |
| A `CommunicationRequest` is the ask, not the message; its fulfilment may produce a `Communication` linked by `basedOn` | [CommunicationRequest R4](https://hl7.org/fhir/R4/communicationrequest.html) | H |
| US Core 9.0.0 (R4) profiles `ServiceRequest`, `CarePlan`, `Goal` and `Encounter`, but not `Appointment`, `Task` or `Communication` | [US Core](https://hl7.org/fhir/us/core/) | H |
| Da Vinci DEQM v5.0.0 defines `$care-gaps`: per patient, a bundle of `MeasureReport`s with open, closed or prospective gaps for a quality measure (e.g. colorectal screening). It is payer- and measure-oriented and needs CQL measures to run | [DEQM Gaps in Care Reporting](https://hl7.org/fhir/us/davinci-deqm/gaps-in-care-reporting.html), [examples](https://hl7.org/fhir/us/davinci-deqm/gaps-examples.html) | H |
| Synthea (R4) exports `CarePlan`, `Encounter`, `Observation`, `DiagnosticReport`, `Immunization`, `Procedure`, `Condition` and more, but not `ServiceRequest`, `Appointment`, `Task` or `Communication` | [Synthea wiki](https://github.com/synthetichealth/synthea/wiki/HL7-FHIR), [AWS HealthLake list](https://docs.aws.amazon.com/healthlake/latest/devguide/reference-healthlake-preloaded-data-types.html) | H |
| HAPI FHIR JPA starter runs locally with `docker run -p 8080:8080 hapiproject/hapi:latest` (H2 database by default, Apache-2.0); it can enable Clinical Reasoning/CQL; `$care-gaps` support was **not confirmed** | [hapi-fhir-jpaserver-starter](https://github.com/hapifhir/hapi-fhir-jpaserver-starter) | H |

**What this means for the design (recommendation):** Synthea gives made-up patients with encounters, labs and care plans, but the "due" items (`ServiceRequest`, `Appointment`, `Task`) must be written by hand as small saved files. A local HAPI server (pinned tag, not `latest`) can serve them, as in the lab project. DEQM `$care-gaps` is too heavy for a first version; simple clinic rules over FHIR data are enough, with `CommunicationRequest`/`Communication` as optional exports.

## 4. Who sells this today

| Seller | What it does | Price found | Source | Confidence |
|---|---|---|---|---|
| Luma Health | Automated recall and care-gap texts tied to the EHR (Epic, athenahealth, Oracle Health); patients book by text; 30+ languages | Not found | [Luma recall one-pager](https://www.lumahealth.io/resource/luma-health-patient-recalls-preventive-care-outreach/) (vendor claim) | M |
| Artera | Text and voice "AI agents" for reminders, care-gap outreach and colonoscopy prep; says its agents give no medical advice; Best in KLAS 2026 (vendor claim). Atlantic Health deployment, 2026: 38% less manual outreach time in month one (vendor/trade claim) | Not found | [Artera AI agents](https://artera.io/ai-agents-in-healthcare), [HIT Consultant, 2026-03-30](https://hitconsultant.net/2026/03/30/atlantic-health-deploy-arteras-ai-agents-for-colonoscopy-patient-outreach/) | M |
| Phreesia | Finds patients due for screenings and wellness visits; outreach by email, text or AI phone call with self-scheduling links | Not found | [Phreesia: close gaps in care](https://www.phreesia.com/products/close-gaps-in-care) (vendor claim) | M |
| Relatient (Dash) | Scheduling, reminders and recall messages | Not found | [Relatient](https://www.relatient.com/solutions/dash-engage/) (vendor claim) | L |
| Klara (ModMed) | Broadcast messages and reminders inside ModMed; recall documentation not found | Not found | [ModMed](https://www.modmed.com/patient-engagement/reminders-surveys/) | L |
| Weave | Phone, texting and reminders for small practices | Pro about $249/month per location (third-party listings disagree: $199–$399) | [Capterra](https://www.capterra.com/p/141842/Weave/pricing/) | M |
| NexHealth | Booking, reminders and recall by text and email, mainly dental | $300/month booking tier on its page; $350–$700/month per location in third-party listings | [NexHealth pricing](https://nexhealth-live-site.pages.dev/pricing), [Capterra](https://www.capterra.com/p/182271/NexHealth/) | L-M |
| Epic | Healthy Planet registries plus bulk MyChart messages; Cheers campaigns; Epic reportedly building AI agents that find gaps and reach out | Bundled with Epic | [Epic](https://www.epic.com/epic/post/closing-preventive-care-gaps-and-improving-population-health-with-automated-outreach), [UChicago handout](https://medchiefs.bsd.uchicago.edu/files/2023/07/Mass-Letter-for-Healthcare-Maintenance.pdf), [Becker's](https://www.beckershospitalreview.com/ehrs/why-epic-is-building-ai-agents-into-its-ehr.html) | M |

**What this means for the design (recommendation):** the market is crowded and automated; sellers compete on EHR integration and scale, and the AI trend is autonomous agents. Where a small tool can stand out: a person approves the exact text of every message (bound by content hash), every message is sent once, follow-ups stop on reply, booking or opt-out, the content guard proves no reason-for-visit leaks, and an evidence pack shows each rule's test. Whether any seller binds approval to exact text: **not found**.

## 5. AI-specific risks

| Finding | Source | Confidence |
|---|---|---|
| In patient-message drafts, 7.1% (11 of 156) of unedited GPT-4 drafts risked severe harm and 0.6% risked death, mostly by not sending the patient to urgent care | Chen et al., [Lancet Digital Health 2024](https://pmc.ncbi.nlm.nih.gov/articles/PMC11829255/) | H |
| In a simulation with 20 primary care physicians, 35–45% of drafts containing errors were sent fully unedited, including a misspelt medication name | npj Digital Medicine 2025, via [Patient Care Online, 2026-07-09](https://www.patientcareonline.com/view/ai-drafts-for-patient-portal-replies-may-increase-primary-care-clinician-editing-burden) | M |
| Across 146,000 messages, LLM drafts were often too long, missed follow-up questions and added irrelevant or inaccurate medical details | [Dartmouth, 2026-07](https://home.dartmouth.edu/news/2026/07/ai-mistakes-can-cost-doctors-time-when-writing-patients) | M |
| Reminders that name a specialty implying a condition (oncology, HIV, sexual health, addiction) can disclose health information to anyone who sees the screen; advice is to name a generic clinic and give date, time and a callback number | Compliance vendor guidance, e.g. [AccountableHQ](https://www.accountablehq.com/post/are-appointment-reminders-hipaa-compliant-what-you-can-and-can-t-include) (vendor claim); [UC San Diego Health policy](https://health.ucsd.edu/policies-notices/text-messaging/): "Sensitive clinical information will not be included." A primary HHS source on specialty names: **not found** | M |
| The AMA recommends patient materials at or below a 6th-grade reading level; NIH figures cited as 6th or 8th grade; only 2.1% of 2,585 journal patient materials met the 6th-grade level | [PMC8205335](https://pmc.ncbi.nlm.nih.gov/articles/PMC8205335) | M |
| Studies of models getting dates, names or times wrong in reminders specifically: **not found** | n/a | n/a |

**What this means for the design (recommendation):** the failures that matter are the ones a reviewer misses: an invented date or time, the wrong name, clinical advice, and a reason for the visit that a family member reading the lock screen should not see. Those are checked in code (dates, times, names and the clinic's details must match the data exactly; no diagnosis, test or specialty words; a reading-level check), not left to the prompt or the approver.

## 6. What this means for the design

Facts are in sections 1–5. Below, **H** marks a hypothesis to test and **R** a recommendation.

1. **R:** the model only words the message. Who is due, and when, comes from rules the clinic writes, shown with their reason, so the tool stays scheduling software, not clinical decision support (FDA, section 2).
2. **R:** the model sees no name or reason for care beyond what the spec allows; the code fills in name, date, time and clinic, and checks the draft contains no other dates, names or clinical words.
3. **R:** the message names no reason for visit, test result or sensitive specialty by default (HIPAA "say little"); the patient's confidential-communications choices are honoured before sending.
4. **R:** a person at the clinic approves the exact text (content hash); the message still says it was drafted with AI and how to reach a person (AB 3030 safe either way).
5. **R:** stop the loop on reply, booking or opt-out, and record the reason. Opt-out is per category (follow-up versus screening recall), matching FCC 26-67.
6. **R:** frequency caps in code: at most 1 message a day and 3 a week per patient per clinic, the TCPA healthcare limits, applied to email too.
7. **R:** first version: email only, through the engine as it is (Mailpit in development).
8. **R:** engine work for later: an SMS provider behind the same `Mailer`-style boundary, with a 160-character limit, STOP/START handling and a local fake SMS sink for tests. Real SMS needs 10DLC registration, which waits for a real clinic.
9. **R:** leave out of the first version: voice calls, two-way chat, self-booking inside the tool, DEQM `$care-gaps`, live EHR write-back, payer quality reporting, and any patient-facing chatbot.
10. **H:** AI-drafted wording is no better for attendance than a well-written template (no trial found); the demo should show both and measure staff time to approve, not claim better outcomes.
11. **H:** a clinic will accept approving each message if the queue shows the reason, the patient and the checked draft on one screen; to test in the spec.

## Gaps

- A trial of LLM-written reminders against templates: **not found**.
- Published effect size of the Geisinger risk-message trials: **not found**.
- The exact eCFR text and citation of the TCPA healthcare exemption: **not verified**.
- The official leginfo text of AB 3030 (page blocked); used a mirror of the statute.
- How FDA treats software that applies a clinic's own recall rules: **not verified**; reasoning above is ours.
- Public prices for Luma, Artera, Phreesia, Relatient and Klara: **not found**.
- Whether `$care-gaps` runs on the HAPI starter: **not confirmed**.
