// cargo run -- preprocess variants ~/Documents/Promotion/varlociraptor-methylation-evaluation/resources/example-genome.fasta --candidates ~/Documents/Promotion/varlociraptor-methylation-evaluation/resources/example-candidates.bcf --bam ~/Documents/Promotion/varlociraptor-methylation-evaluation/resources/example-reads.bam > ~/Documents/Promotion/varlociraptor-methylation-evaluation/resources/observations.bcf

use super::ToVariantRepresentation;
use crate::estimation::alignment_properties::AlignmentProperties;
use crate::variants::evidence::realignment::Realignable;
use crate::variants::model;

use crate::variants::evidence::observations::read_observation::Strand;
use rust_htslib::bam::record::Aux;
use crate::variants::types::{
    AlleleSupport, AlleleSupportBuilder, Overlap, SingleEndEvidence, SingleLocus, Variant,
};
use anyhow::Result;
use bio::stats::{LogProb, Prob};
use bio_types::genome::{self, AbstractInterval, AbstractLocus};
use num_traits::ToPrimitive;

#[derive(Debug)]
pub(crate) struct Methylation {
    locus: SingleLocus,
    
}

impl Methylation {
    pub(crate) fn new(locus: genome::Locus) -> Self {
        Methylation {
            locus: SingleLocus::new(genome::Interval::new(
                locus.contig().to_owned(),
                locus.pos()..locus.pos() + 2,
            )),
        }
    }
}

use log::{warn, error};

fn methylated_cs(read: &SingleEndEvidence) -> Result<Vec<usize>, String> {
    let mm_tag = read.aux(b"MM").map_err(|e| e.to_string())?;
    
    if let Aux::String(tag_value) = mm_tag {
        let mut basemods_value = tag_value.to_owned();
        
        // Überprüfen, ob der Tag nicht leer ist
        if !basemods_value.is_empty() {
            basemods_value.pop();
            warn!("MM:Z value: {}", basemods_value);
            
            if let Some(methylated_part) = basemods_value.strip_prefix("C+m,") {
                let mut meth_pos = 0;
                let methylated_cs: Vec<usize> = methylated_part.split(',')
                    .filter_map(|position_str| {
                        position_str.parse::<usize>().ok().map(|position| {
                            meth_pos += position + 1;
                            meth_pos
                        })
                    })
                    .collect();
                let pos_cs= String::from_utf8_lossy(read.seq().encoded).to_string();
                  /*   .char_indices()
                    .filter(|(_, c)| *c == 'C')
                    .map(|(index, _)| index)
                    .collect(); */
                // let pos_methylated_cs = pos_cs.filter((|_|, pos)| )
                return Ok(methylated_cs);
            }
        }
    } else {
        error!("Tag is not of type String");
    }
    Err("Error while obtaining MM:Z tag".to_string())
}

impl Variant for Methylation {
    type Evidence = SingleEndEvidence;
    type Loci = SingleLocus;

    fn is_imprecise(&self) -> bool {
        false
    }

    /// Determine whether the evidence is suitable to assessing probabilities
    /// (i.e. overlaps the variant in the right way).
    ///
    /// # Returns
    ///
    /// The index of the loci for which this evidence is valid, `None` if invalid.
    // ! Warum geben wir vec![0] zurück und nicht die Locusposition?
    fn is_valid_evidence(
        &self,
        evidence: &SingleEndEvidence,
        _: &AlignmentProperties,
    ) -> Option<Vec<usize>> {
        if let Overlap::Enclosing = self.locus.overlap(evidence, false) {
            Some(vec![0])
        } else {
            None
        }
    }

    /// Return variant loci.
    fn loci(&self) -> &SingleLocus {
        &self.locus
    }

    fn allele_support(
        &self,
        read: &SingleEndEvidence,
        _alignment_properties: &AlignmentProperties,
        _alt_variants: &[Box<dyn Realignable>],
    ) -> Result<Option<AlleleSupport>> {
        // qpos: Position im Read, an der das C steht wenn es die nicht gibt, wird der Read nicht betrachtet und der else Block wird ausgeführt.
        
        if let Some(qpos) = read
            .cigar_cached()
            .unwrap()
            // TODO expect u64 in read_pos
            .read_pos(self.locus.range().start as u32, false, false)?
        {
            // TODO Wenn read_base kein C ist muss ich mir was ausdenken? Was ist, wenn die nächte Base im Read kein G ist?
            let read_base = unsafe { read.seq().decoded_base_unchecked(qpos as usize) };
            // TODO base qual muss ich noch irgendwie einbringen?
            let base_qual = unsafe { *read.qual().get_unchecked(qpos as usize) };
            // Hole info aus MM File, ob das C methyliert ist.
            let meth_pos = methylated_cs(read).unwrap();
            let is_meth = meth_pos.contains(&qpos.to_usize().unwrap());
            // TODO prob alt berechne ich über den ML String im BAM File
            // ? let prob_alt = prob_read_base(read_base, self.alt_base, base_qual);
            // ! Wo bekomme ich die Wkeit für prob_ref her? Ist es erstmal einfach 1 - prob_alt? Was wäre dann o_i?
            // ? let prob_ref = prob_read_base(read_base, non_alt_base, base_qual);
            let prob_ref = LogProb::from(Prob(1.0));
            let prob_alt = LogProb::from(Prob(0.0));
            // ! Um strand muss ich mich noch kuemmern
            let strand = Strand::no_strand_info();
            Ok(Some(
                AlleleSupportBuilder::default()
                    .prob_ref_allele(prob_ref)
                    .prob_alt_allele(prob_alt)
                    .strand(strand)
                    .read_position(Some(qpos))
                    // ! Um third allele muss ich mich noch kuemmern
                    .third_allele_evidence(None)
                    .build()
                    .unwrap(),
            ))
        } else {
            // a read that spans an SNV might have the respective position in the
            // reference skipped (Cigar op 'N'), and the library should not choke on those reads
            // but instead needs to know NOT to add those reads (as observations) further up
            Ok(None)
        }
    }

    /// Calculate probability to sample a record length like the given one from the alt allele.
    fn prob_sample_alt(&self, _: &SingleEndEvidence, _: &AlignmentProperties) -> LogProb {
        LogProb::ln_one()
    }
}

impl ToVariantRepresentation for Methylation {
    fn to_variant_representation(&self) -> model::Variant {
        model::Variant::Methylation()
    }
}

/*
! Die Argumente alignment_properties, alt_variants brauchen wir eigentlich nicht, da wir nicht realignen, müssen diese jedoch übergeben, um den Trait richtig zu implementieren.
fn allele_support(
        &self,
        read: &SingleEndEvidence,
        alignment_properties: &AlignmentProperties,
        alt_variants: &[Box<dyn Realignable>],
    ) -> Result<Option<AlleleSupport>> {
        ! 1. Beginn: Diesen Block brauchen wir nicht, da wir bei Methylierung eh nicht realignen
        if utils::contains_indel_op(&**read) {
            // METHOD: reads containing indel operations should always be realigned,
            // as their support or non-support of the SNV might be an artifact
            // of the aligner.
            Ok(Some(self.realigner.borrow_mut().allele_support(
                &**read,
                [&self.locus].iter(),
                self,
                alt_variants,
                alignment_properties,
            )?))
        ! 1. End
        ! 2. if let similar to match. Wenn read of value Some: Packe read.blablabla in qpos und führe Block aus. Wenn read = None: Überspringe Block
        !   qpos: Locusposition im Read
        ! Bsp: AACTGCA Locus: 2 -> C
        !        CTG   Qpos: 0
        } else if let Some(qpos) = read
            .cigar_cached()
            .unwrap()
            // TODO expect u64 in read_pos
            .read_pos(self.locus.range().start as u32, false, false)?
        {
            ! Nukleotid im Read an der Stelle qpos
            let read_base = unsafe { read.seq().decoded_base_unchecked(qpos as usize) };
            ! Wie sicher icht es der richtige Read
            let base_qual = unsafe { *read.qual().get_unchecked(qpos as usize) };
            ? Wie funktioniert prob_read_base?
            let prob_alt = prob_read_base(read_base, self.alt_base, base_qual);
            ! Brauchen wir nicht
            let mut is_third_allele = false;

            // METHOD: instead of considering the actual REF base, we assume that REF is whatever
            // base the read has at this position (if not the ALT base). This way, we avoid biased
            // allele frequencies at sites with multiple alternative alleles.
            // Note that this is an approximation. The real solution would be to have multiple allele
            // frequency variables in the likelihood function, but that would be computationally
            // more demanding (leading to a combinatorial explosion).
            // However, the approximation is pretty accurate, because it will only matter for true
            // multiallelic cases. Sequencing errors won't have a severe effect on the allele frequencies
            // because they are too rare.
            ! Können wir so abkürzen: Non_alt_base = read_base
            let non_alt_base = if read_base != self.alt_base {
                is_third_allele = read_base != self.ref_base;
                read_base
            } else {
                self.ref_base
            };
            ? Wie funktioniert prob_read_base?
            let prob_ref = prob_read_base(read_base, non_alt_base, base_qual);
            ! Brauchen wir nicht
            let strand = if prob_ref != prob_alt {
                Strand::from_record_and_pos(read, qpos as usize)?
            } else {
                // METHOD: if record is not informative, we don't want to
                // retain its information (e.g. strand).
                Strand::no_strand_info()
            };
            ! Muss ich mal gucken, wie ich das ohne strand und third_allele machen kann.
            Ok(Some(
                AlleleSupportBuilder::default()
                    .prob_ref_allele(prob_ref)
                    .prob_alt_allele(prob_alt)
                    .strand(strand)
                    .read_position(Some(qpos))
                    .third_allele_evidence(if is_third_allele {
                        Some(EditDistance(1))
                    } else {
                        None
                    })
                    .build()
                    .unwrap(),
            ))
        ! Irgendetwas ist schief gelaufen
        } else {
            // a read that spans an SNV might have the respective position in the
            // reference skipped (Cigar op 'N'), and the library should not choke on those reads
            // but instead needs to know NOT to add those reads (as observations) further up
            Ok(None)
        }
    }


*/
